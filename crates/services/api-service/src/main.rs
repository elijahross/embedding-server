pub mod ai;
mod cache;
pub mod config;
pub mod error;
mod log;
mod middleware;
mod routes;
pub mod types;

pub use self::error::{Error, Result};
use crate::cache::AppState;
use crate::middleware::mw_auth::{UserToken, ctx_resolver, request_auth};
use crate::middleware::mw_response::mw_response_map;
use axum::middleware::from_fn;
use axum::{Router, extract::Extension, serve};
use clap::Parser;
use lib_core::database::ModelManager;
use lib_embedding::DType;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::time::Duration;
use tower_cookies::CookieManagerLayer;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tracing::info;
use tracing_subscriber::EnvFilter;

// Use mimalloc as the global allocator on non-linux platforms for better memory usage on long running jobs
#[cfg(not(target_os = "linux"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// region: Model Arguments
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The name of the model to load.
    /// Can be a MODEL_ID as listed on <https://hf.co/models> like
    /// `BAAI/bge-large-en-v1.5`.
    /// Or it can be a local directory containing the necessary files
    /// as saved by `save_pretrained(...)` methods of transformers
    #[clap(default_value = "./Qwen3-Embedding-0.6B", long, env)]
    model_id: String,

    /// The actual revision of the model if you're referring to a model
    /// on the hub. You can use a specific commit id or a branch like `refs/pr/2`.
    #[clap(long, env)]
    revision: Option<String>,

    /// Optionally control the number of tokenizer workers used for payload tokenization, validation
    /// and truncation.
    /// Default to the number of CPU cores on the machine.
    #[clap(default_value = "1", long, env)]
    tokenization_workers: Option<usize>,

    /// The dtype to be forced
    #[clap(default_value = "float16", long, env, value_enum)]
    dtype: Option<DType>,

    /// Optionally control the pooling method for embedding models.
    ///
    /// If `pooling` is not set, the pooling configuration will be parsed from the
    /// model `1_Pooling/config.json` configuration.
    ///
    /// If `pooling` is set, it will override the model pooling configuration
    ///

    #[clap(long, env, value_enum)]
    pooling: Option<lib_embedding::Pool>,

    /// The maximum amount of concurrent requests for this particular deployment.
    /// Having a low limit will refuse clients requests instead of having them
    /// wait for too long and is usually good to handle backpressure correctly.
    #[clap(default_value = "1", long, env)]
    max_concurrent_requests: usize,

    /// **IMPORTANT** This is one critical control to allow maximum usage
    /// of the available hardware.
    ///
    /// This represents the total amount of potential tokens within a batch.
    ///
    /// For `max_batch_tokens=1000`, you could fit `10` queries of `total_tokens=100`
    /// or a single query of `1000` tokens.
    ///
    /// Overall this number should be the largest possible until the model is compute bound.
    /// Since the actual memory overhead depends on the model implementation,
    /// text-embeddings-inference cannot infer this number automatically.
    #[clap(default_value = "1384", long, env)]
    max_batch_tokens: usize,

    /// Optionally control the maximum number of individual requests in a batch
    #[clap(default_value = "5", long, env)]
    max_batch_requests: Option<usize>,

    /// Control the maximum number of inputs that a client can send in a single request
    #[clap(default_value = "2", long, env)]
    max_client_batch_size: usize,

    /// Automatically truncate inputs that are longer than the maximum supported size
    ///
    /// Unused for gRPC servers
    #[clap(long, env)]
    auto_truncate: bool,

    /// The name of the prompt that should be used by default for encoding. If not set, no prompt
    /// will be applied.
    ///
    /// Must be a key in the `sentence-transformers` configuration `prompts` dictionary.
    ///
    /// For example if ``default_prompt_name`` is "query" and the ``prompts`` is {"query": "query: ", ...},
    /// then the sentence "What is the capital of France?" will be encoded as
    /// "query: What is the capital of France?" because the prompt text will be prepended before
    /// any text to encode.
    ///
    /// The argument '--default-prompt-name <DEFAULT_PROMPT_NAME>' cannot be used with
    /// '--default-prompt <DEFAULT_PROMPT>`
    #[clap(long, env, conflicts_with = "default_prompt")]
    default_prompt_name: Option<String>,

    /// The prompt that should be used by default for encoding. If not set, no prompt
    /// will be applied.
    ///
    /// For example if ``default_prompt`` is "query: " then the sentence "What is the capital of
    /// France?" will be encoded as "query: What is the capital of France?" because the prompt
    /// text will be prepended before any text to encode.
    ///
    /// The argument '--default-prompt <DEFAULT_PROMPT>' cannot be used with
    /// '--default-prompt-name <DEFAULT_PROMPT_NAME>`
    #[clap(long, env, conflicts_with = "default_prompt_name")]
    default_prompt: Option<String>,

    /// Optionally, define the path to the Dense module required for some embedding models.
    ///
    /// Some embedding models require an extra `Dense` module which contains a single Linear layer
    /// and an activation function. By default, those `Dense` modules are stored under the `2_Dense`
    /// directory, but there might be cases where different `Dense` modules are provided, to
    /// convert the pooled embeddings into different dimensions, available as `2_Dense_<dims>` e.g.
    /// https://huggingface.co/NovaSearch/stella_en_400M_v5.
    ///
    /// Note that this argument is optional, only required to be set if there is no `modules.json`
    /// file or when you want to override a single Dense module path, only when running with the
    /// `candle` backend.
    #[clap(long, env)]
    dense_path: Option<String>,

    /// [DEPRECATED IN FAVOR OF `--hf-token`] Your Hugging Face Hub token
    #[clap(long, env, hide = true)]
    hf_api_token: Option<String>,

    /// Your Hugging Face Hub token
    #[clap(long, env, conflicts_with = "hf_api_token")]
    hf_token: Option<String>,

    /// The IP address to listen on
    #[clap(default_value = "0.0.0.0", long, env)]
    hostname: String,

    /// The port to listen on.
    #[clap(default_value = "8080", long, short, env)]
    port: u16,

    /// The name of the unix socket some lib_embedding backends will use as they
    /// communicate internally with gRPC.
    #[clap(default_value = "/tmp/inference-server", long, env)]
    uds_path: String,

    /// The location of the huggingface hub cache.
    /// Used to override the location if you want to provide a mounted disk for instance
    #[clap(long, env)]
    huggingface_hub_cache: Option<String>,

    /// Payload size limit in bytes
    ///
    /// Default is 2MB
    #[clap(default_value = "2000000", long, env)]
    payload_limit: usize,

    /// Set an api key for request authorization.
    ///
    /// By default the server responds to every request. With an api key set, the requests must have the Authorization header set with the api key as Bearer token.
    #[clap(long, env)]
    api_key: Option<String>,

    /// Outputs the logs in JSON format (useful for telemetry)
    #[clap(long, env)]
    json_output: bool,

    // Whether or not to include the log trace through spans
    #[clap(long, env)]
    disable_spans: bool,

    /// The grpc endpoint for opentelemetry. Telemetry is sent to this endpoint as OTLP over gRPC.
    /// e.g. `http://localhost:4317`
    #[clap(long, env)]
    otlp_endpoint: Option<String>,

    /// The service name for opentelemetry.
    /// e.g. `s3-embedding.server`
    #[clap(default_value = "s3-embedding.server", long, env)]
    otlp_service_name: String,

    /// Unused for gRPC servers
    #[clap(long, env)]
    cors_allow_origin: Option<Vec<String>>,
}

// endregion: Arguments
/// App Configuration

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("Parsing Args");
    // Pattern match configuration
    let args: Args = Args::parse();

    tracing::info!("{:?}", args);

    // Hack to trim pages regularly
    // see: https://www.algolia.com/blog/engineering/when-allocators-are-hoarding-your-precious-memory/
    // and: https://github.com/huggingface/text-embeddings-inference/issues/156
    #[cfg(target_os = "linux")]
    tokio::spawn(async move {
        use tokio::time::Duration;
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            unsafe {
                libc::malloc_trim(0);
            }
        }
    });

    let token = args.hf_token.or(args.hf_api_token);
    let api_key = args.api_key.clone();
    info!("Starting AI Inference");
    let (infer, info) = ai::run(
        args.model_id,
        args.revision,
        args.tokenization_workers,
        args.dtype,
        args.pooling,
        args.max_concurrent_requests,
        args.max_batch_tokens,
        args.max_batch_requests,
        args.max_client_batch_size,
        args.auto_truncate,
        args.default_prompt,
        args.default_prompt_name,
        args.dense_path,
        token,
        Some(args.uds_path),
        args.huggingface_hub_cache,
        args.otlp_endpoint,
        args.otlp_service_name,
    )
    .await?;

    info!("Initializing Environment");
    let ip_addr: Ipv4Addr = args
        .hostname
        .parse()
        .expect("Invalid IP address in hostname");
    let addr = SocketAddr::from((ip_addr, args.port));
    let listener = TcpListener::bind(&addr).await.unwrap();

    // Initialize the model manager for database access
    let mm = ModelManager::new().await?;
    // Create application context
    let app_state = AppState::new(Arc::new(mm.clone()), Arc::new(info), Arc::new(infer)).await?;

    // Rate limiting Configuration, limits are tied to the provided API key (can be switched to IP address or userId)
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(80) // Increase the rate limit to 40 requests per second
            .burst_size(50) // Allow a burst of up to 50 requests
            .key_extractor(UserToken)
            .finish()
            .unwrap(),
    );
    let governor_limiter = governor_conf.limiter().clone();

    // A separate background task to clean up rate limiting storage
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(60));
            governor_limiter.retain_recent();
        }
    });

    // API Routes tied with rate limiting and authentication middleware
    let routes_api = Router::new()
        .merge(routes::cron::serve_cron())
        .merge(routes::embed::serve_embed())
        .route_layer(from_fn(request_auth))
        .layer(GovernorLayer {
            config: governor_conf,
        });

    // Global routes with CORS, cookies, file serving routes should be implemented here
    let global_routes = Router::new()
        .nest("/api/v1", routes_api)
        .layer(axum::middleware::from_fn_with_state(api_key, ctx_resolver))
        .layer(axum::middleware::map_response(mw_response_map))
        .layer(CookieManagerLayer::new())
        .layer(Extension(app_state.clone()));

    info!("Server started on: http://{}", addr);
    serve(listener, global_routes.into_make_service())
        .await
        .unwrap();

    Ok(())
}
