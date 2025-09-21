pub mod download;
pub mod infer;
pub mod queue;
pub mod tokenization;

use crate::ai::download::{ST_CONFIG_NAMES, download_artifacts};
use crate::ai::infer::Infer;
use crate::ai::queue::Queue;
use crate::ai::tokenization::Tokenization;
use crate::error::{self, Error, Result};
use axum::http::HeaderMap;
use hf_hub::api::tokio::ApiBuilder;
use hf_hub::{Repo, RepoType};
use lib_embedding::{DType, Pool};
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tokenizers::processors::sequence::Sequence;
use tokenizers::processors::template::TemplateProcessing;
use tokenizers::{PostProcessorWrapper, Tokenizer};
use tracing::{Span, error, info, instrument};

/// Create entrypoint
#[allow(clippy::too_many_arguments)]
pub async fn run(
    model_id: String,
    revision: Option<String>,
    tokenization_workers: Option<usize>,
    dtype: Option<DType>,
    pooling: Option<Pool>,
    max_concurrent_requests: usize,
    max_batch_tokens: usize,
    max_batch_requests: Option<usize>,
    max_client_batch_size: usize,
    auto_truncate: bool,
    default_prompt: Option<String>,
    default_prompt_name: Option<String>,
    dense_path: Option<String>,
    hf_token: Option<String>,
    uds_path: Option<String>,
    huggingface_hub_cache: Option<String>,
    otlp_endpoint: Option<String>,
    otlp_service_name: String,
) -> Result<(Infer, Info)> {
    let model_id_path = Path::new(&model_id);
    let (model_root, api_repo) = if model_id_path.exists() && model_id_path.is_dir() {
        // Using a local model
        (model_id_path.to_path_buf(), None)
    } else {
        let mut builder = ApiBuilder::from_env()
            .with_progress(false)
            .with_token(hf_token);

        if let Some(cache_dir) = huggingface_hub_cache {
            builder = builder.with_cache_dir(cache_dir.into());
        }

        if let Ok(origin) = std::env::var("HF_HUB_USER_AGENT_ORIGIN") {
            builder = builder.with_user_agent("origin", origin.as_str());
        }

        let api = builder.build().unwrap();
        let api_repo = api.repo(Repo::with_revision(
            model_id.clone(),
            RepoType::Model,
            revision.clone().unwrap_or("main".to_string()),
        ));

        // Download model from the Hub
        (
            download_artifacts(&api_repo, pooling.is_none()).await?,
            Some(api_repo),
        )
    };

    // Load config
    let config_path = model_root.join("config.json");
    let config = fs::read_to_string(config_path)
        .map_err(|_err| Error::Custom("`config.json` not found".into()))?;
    let config: ModelConfig = serde_json::from_str(&config)
        .map_err(|_err| Error::Custom("Failed to parse `config.json`".into()))?;

    // Set model type from config
    let backend_model_type = get_backend_model_type(&config, &model_root, pooling)?;

    // Info model type
    let model_type = match &backend_model_type {
        lib_embedding::ModelType::Classifier => {
            let id2label = config.id2label.clone().ok_or(Error::Custom(
                "`config.json` does not contain `id2label`".into(),
            ))?;
            let n_classes = id2label.len();
            let classifier_model = ClassifierModel {
                id2label,
                label2id: config.label2id.clone().ok_or(Error::Custom(
                    "`config.json` does not contain `label2id`".into(),
                ))?,
            };
            if n_classes > 1 {
                ModelType::Classifier(classifier_model)
            } else {
                ModelType::Reranker(classifier_model)
            }
        }
        lib_embedding::ModelType::Embedding(pool) => ModelType::Embedding(EmbeddingModel {
            pooling: pool.to_string(),
        }),
    };

    // Load tokenizer
    let tokenizer_path = model_root.join("tokenizer.json");
    let mut tokenizer = Tokenizer::from_file(tokenizer_path).expect(
        "tokenizer.json not found. text-embeddings-inference only supports fast tokenizers",
    );
    tokenizer.with_padding(None);

    // Qwen2 updates the post processor manually instead of into the tokenizer.json...
    // https://huggingface.co/Alibaba-NLP/gte-Qwen2-1.5B-instruct/blob/main/tokenization_qwen.py#L246
    if config.model_type == "qwen2" {
        let template = TemplateProcessing::builder()
            .try_single("$A:0 <|endoftext|>:0")
            .unwrap()
            .try_pair("$A:0 <|endoftext|>:0 $B:1 <|endoftext|>:1")
            .unwrap()
            .special_tokens(vec![("<|endoftext|>", 151643)])
            .build()
            .unwrap();

        match tokenizer.get_post_processor() {
            None => tokenizer.with_post_processor(Some(template)),
            Some(post_processor) => {
                let post_processor = Sequence::new(vec![
                    post_processor.clone(),
                    PostProcessorWrapper::Template(template),
                ]);
                tokenizer.with_post_processor(Some(post_processor))
            }
        };
    }

    // Position IDs offset. Used for Roberta and camembert.
    let position_offset = if &config.model_type == "xlm-roberta"
        || &config.model_type == "camembert"
        || &config.model_type == "roberta"
    {
        config.pad_token_id + 1
    } else {
        0
    };

    // Try to load ST Config
    let mut st_config: Option<STConfig> = None;
    for name in ST_CONFIG_NAMES {
        let config_path = model_root.join(name);
        if let Ok(config) = fs::read_to_string(config_path) {
            st_config = Some(
                serde_json::from_str(&config)
                    .map_err(|_err| Error::Custom(format!("Failed to parse `{}`", name)))?,
            );
            break;
        }
    }
    let max_input_length = match st_config {
        Some(config) => config.max_seq_length,
        None => {
            tracing::warn!("Could not find a Sentence Transformers config");
            config.max_position_embeddings - position_offset
        }
    };
    tracing::info!("Maximum number of tokens per request: {max_input_length}");

    let tokenization_workers = tokenization_workers.unwrap_or_else(num_cpus::get);

    // Try to load new ST Config
    let mut new_st_config: Option<NewSTConfig> = None;
    let config_path = model_root.join("config_sentence_transformers.json");
    if let Ok(config) = fs::read_to_string(config_path.clone()) {
        new_st_config = Some(serde_json::from_str(&config).map_err(|_err| {
            Error::Custom(format!("Failed to parse `{}`", config_path.display()))
        })?);
    }
    let prompts = new_st_config.and_then(|c| c.prompts);
    let default_prompt = if let Some(default_prompt_name) = default_prompt_name.as_ref() {
        match &prompts {
            None => {
                error!(
                    "`default-prompt-name` is set to `{}` but we could not find a `config_sentence_transformers.json` file with prompts.",
                    default_prompt_name
                );
                None
            }
            Some(prompts) if !prompts.contains_key(default_prompt_name) => {
                error!(
                    "`default-prompt-name` is set to `{}` but it was not found in the Sentence Transformers prompts. Available prompts: {:?}",
                    default_prompt_name,
                    prompts.keys()
                );
                None
            }
            Some(prompts) => prompts.get(default_prompt_name).cloned(),
        }
    } else {
        default_prompt
    };

    // Tokenization logic
    let tokenization = Tokenization::new(
        tokenization_workers,
        tokenizer,
        max_input_length,
        position_offset,
        default_prompt,
        prompts,
    );

    // NOTE: `gemma3_text` won't support Float16 but only Float32, given that with `candle-cuda`
    // feature, the default `Dtype::Float16` this overrides that to prevent issues when running a
    // `gemma3_text` model without specifying a `--dtype`
    let dtype = if dtype.is_none() && config.model_type == "gemma3_text" {
        DType::Float32
    } else {
        dtype.unwrap_or_default()
    };

    // Create backend
    tracing::info!("Starting model backend");
    let backend = lib_embedding::InferenceBackend::new(
        model_root,
        api_repo,
        dtype.clone(),
        backend_model_type,
        dense_path,
        uds_path.unwrap_or("/tmp/text-embeddings-inference-server".to_string()),
        otlp_endpoint.clone(),
        otlp_service_name.clone(),
    )
    .await
    .map_err(|err| {
        Error::Custom(format!(
            "Failed to create model backend. Please make sure the model is supported. Error: {err}"
        ))
    })?;
    backend.health().await.map_err(|err| {
        Error::Custom(format!(
            "Model backend is not healthy. Please make sure the model is supported. Error: {err}"
        ))
    })?;

    tracing::info!("Warming up model");
    backend
        .warmup(max_input_length, max_batch_tokens, max_batch_requests)
        .await
        .map_err(|err| {
            Error::Custom(format!(
                "Model backend is not healthy. Please make sure the model is supported. Error: {err}"
            ))
        })?;

    let max_batch_requests = backend
        .max_batch_size
        .inspect(|&s| {
            tracing::warn!("Backend does not support a batch size > {s}");
            tracing::warn!("forcing `max_batch_requests={s}`");
        })
        .or(max_batch_requests);

    // Queue logic
    let queue = Queue::new(
        backend.padded_model,
        max_batch_tokens,
        max_batch_requests,
        max_concurrent_requests,
    );

    // Create infer task
    let infer = Infer::new(tokenization, queue, max_concurrent_requests, backend);

    // Endpoint info
    let info = Info {
        model_id,
        model_sha: revision,
        model_dtype: dtype.to_string(),
        model_type,
        max_concurrent_requests,
        max_input_length,
        max_batch_tokens,
        tokenization_workers,
        max_batch_requests,
        max_client_batch_size,
        auto_truncate,
        version: env!("CARGO_PKG_VERSION"),
        sha: option_env!("VERGEN_GIT_SHA"),
        docker_label: option_env!("DOCKER_LABEL"),
    };
    Ok((infer, info))
}

fn get_backend_model_type(
    config: &ModelConfig,
    model_root: &Path,
    pooling: Option<lib_embedding::Pool>,
) -> Result<lib_embedding::ModelType> {
    for arch in &config.architectures {
        // Edge case affecting `Alibaba-NLP/gte-multilingual-base` and possibly other fine-tunes of
        // the same base model. More context at https://huggingface.co/Alibaba-NLP/gte-multilingual-base/discussions/7
        if arch == "NewForTokenClassification"
            && (config.id2label.is_none() | config.label2id.is_none())
        {
            tracing::warn!(
                "Provided `--model-id` is likely an AlibabaNLP GTE model, but the `config.json` contains the architecture `NewForTokenClassification` but it doesn't contain the `id2label` and `label2id` mapping, so `NewForTokenClassification` architecture will be ignored."
            );
            continue;
        }

        if Some(lib_embedding::Pool::Splade) == pooling && arch.ends_with("MaskedLM") {
            return Ok(lib_embedding::ModelType::Embedding(
                lib_embedding::Pool::Splade,
            ));
        } else if arch.ends_with("Classification") {
            if pooling.is_some() {
                tracing::warn!(
                    "`--pooling` arg is set but model is a classifier. Ignoring `--pooling` arg."
                );
            }
            return Ok(lib_embedding::ModelType::Classifier);
        }
    }

    if Some(lib_embedding::Pool::Splade) == pooling {
        return Err(Error::Custom(
            "Splade pooling is not supported: model is not a ForMaskedLM model".to_string(),
        ));
    }

    // Set pooling
    let pool = match pooling {
        Some(pool) => pool,
        None => {
            // Load pooling config
            let config_path = model_root.join("1_Pooling/config.json");

            match fs::read_to_string(config_path) {
                Ok(config) => {
                    let config: PoolConfig = serde_json::from_str(&config)?;
                    Pool::try_from(config)?
                }
                Err(err) => {
                    if !config.model_type.to_lowercase().contains("bert") {
                        return Err(Error::Custom(format!(
                            "`--pooling` arg is not set and we could not find a pooling configuration (`1_Pooling/config.json`) for this model. Please set the `--pooling` arg. Error: {err}"
                        )));
                    }
                    tracing::warn!(
                        "The `--pooling` arg is not set and we could not find a pooling configuration (`1_Pooling/config.json`) for this model but the model is a BERT variant. Defaulting to `CLS` pooling."
                    );
                    lib_embedding::Pool::Cls
                }
            }
        }
    };

    Ok(lib_embedding::ModelType::Embedding(pool))
}

#[derive(Debug, Deserialize)]
pub struct ModelConfig {
    pub architectures: Vec<String>,
    pub model_type: String,
    #[serde(alias = "n_positions")]
    pub max_position_embeddings: usize,
    #[serde(default)]
    pub pad_token_id: usize,
    pub id2label: Option<HashMap<String, String>>,
    pub label2id: Option<HashMap<String, usize>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PoolConfig {
    pooling_mode_cls_token: bool,
    pooling_mode_mean_tokens: bool,
    #[serde(default)]
    pooling_mode_lasttoken: bool,
}

impl TryFrom<PoolConfig> for Pool {
    type Error = Error;

    fn try_from(config: PoolConfig) -> Result<Self> {
        if config.pooling_mode_cls_token {
            return Ok(Pool::Cls);
        }
        if config.pooling_mode_mean_tokens {
            return Ok(Pool::Mean);
        }
        if config.pooling_mode_lasttoken {
            return Ok(Pool::LastToken);
        }
        Err(Error::Custom(format!("Pooling config {config:?} is not supported")).into())
    }
}

#[derive(Debug, Deserialize)]
pub struct STConfig {
    pub max_seq_length: usize,
}

#[derive(Debug, Deserialize)]
pub struct NewSTConfig {
    pub prompts: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "http", derive(utoipa::ToSchema))]
pub struct EmbeddingModel {
    #[cfg_attr(feature = "http", schema(example = "cls"))]
    pub pooling: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "http", derive(utoipa::ToSchema))]
pub struct ClassifierModel {
    #[cfg_attr(feature = "http", schema(example = json!({"0": "LABEL"})))]
    pub id2label: HashMap<String, String>,
    #[cfg_attr(feature = "http", schema(example = json!({"LABEL": 0})))]
    pub label2id: HashMap<String, usize>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "http", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ModelType {
    Classifier(ClassifierModel),
    Embedding(EmbeddingModel),
    Reranker(ClassifierModel),
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "http", derive(utoipa::ToSchema))]
pub struct Info {
    /// Model info
    #[cfg_attr(feature = "http", schema(example = "thenlper/gte-base"))]
    pub model_id: String,
    #[cfg_attr(
        feature = "http",
        schema(nullable = true, example = "fca14538aa9956a46526bd1d0d11d69e19b5a101")
    )]
    pub model_sha: Option<String>,
    #[cfg_attr(feature = "http", schema(example = "float16"))]
    pub model_dtype: String,
    pub model_type: ModelType,
    /// Router Parameters
    #[cfg_attr(feature = "http", schema(example = "128"))]
    pub max_concurrent_requests: usize,
    #[cfg_attr(feature = "http", schema(example = "512"))]
    pub max_input_length: usize,
    #[cfg_attr(feature = "http", schema(example = "2048"))]
    pub max_batch_tokens: usize,
    #[cfg_attr(
        feature = "http",
        schema(nullable = true, example = "null", default = "null")
    )]
    pub max_batch_requests: Option<usize>,
    #[cfg_attr(feature = "http", schema(example = "32"))]
    pub max_client_batch_size: usize,
    pub auto_truncate: bool,
    #[cfg_attr(feature = "http", schema(example = "4"))]
    pub tokenization_workers: usize,
    /// Router Info
    #[cfg_attr(feature = "http", schema(example = "0.5.0"))]
    pub version: &'static str,
    #[cfg_attr(feature = "http", schema(nullable = true, example = "null"))]
    pub sha: Option<&'static str>,
    #[cfg_attr(feature = "http", schema(nullable = true, example = "null"))]
    pub docker_label: Option<&'static str>,
}

pub struct ResponseMetadata {
    compute_chars: usize,
    compute_tokens: usize,
    start_time: Instant,
    tokenization_time: Duration,
    queue_time: Duration,
    inference_time: Duration,
}

impl ResponseMetadata {
    pub fn new(
        compute_chars: usize,
        compute_tokens: usize,
        start_time: Instant,
        tokenization_time: Duration,
        queue_time: Duration,
        inference_time: Duration,
    ) -> Self {
        Self {
            compute_chars,
            compute_tokens,
            start_time,
            tokenization_time,
            queue_time,
            inference_time,
        }
    }

    pub fn record_span(&self, span: &Span) {
        // Tracing metadata
        span.record("compute_chars", self.compute_chars);
        span.record("compute_tokens", self.compute_tokens);
        span.record("total_time", format!("{:?}", self.start_time.elapsed()));
        span.record("tokenization_time", format!("{:?}", self.tokenization_time));
        span.record("queue_time", format!("{:?}", self.queue_time));
        span.record("inference_time", format!("{:?}", self.inference_time));
    }

    pub fn record_metrics(&self) {
        // Metrics
        let histogram = metrics::histogram!("te_request_duration");
        histogram.record(self.start_time.elapsed().as_secs_f64());
        let histogram = metrics::histogram!("te_request_tokenization_duration");
        histogram.record(self.tokenization_time.as_secs_f64());
        let histogram = metrics::histogram!("te_request_queue_duration");
        histogram.record(self.queue_time.as_secs_f64());
        let histogram = metrics::histogram!("te_request_inference_duration");
        histogram.record(self.inference_time.as_secs_f64());
    }
}

impl From<ResponseMetadata> for HeaderMap {
    fn from(value: ResponseMetadata) -> Self {
        // Headers
        let mut headers = HeaderMap::new();
        headers.insert("x-compute-type", "gpu+optimized".parse().unwrap());
        headers.insert(
            "x-compute-time",
            value
                .start_time
                .elapsed()
                .as_millis()
                .to_string()
                .parse()
                .unwrap(),
        );
        headers.insert(
            "x-compute-characters",
            value.compute_chars.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-compute-tokens",
            value.compute_tokens.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-total-time",
            value
                .start_time
                .elapsed()
                .as_millis()
                .to_string()
                .parse()
                .unwrap(),
        );
        headers.insert(
            "x-tokenization-time",
            value
                .tokenization_time
                .as_millis()
                .to_string()
                .parse()
                .unwrap(),
        );
        headers.insert(
            "x-queue-time",
            value.queue_time.as_millis().to_string().parse().unwrap(),
        );
        headers.insert(
            "x-inference-time",
            value
                .inference_time
                .as_millis()
                .to_string()
                .parse()
                .unwrap(),
        );
        headers
    }
}

// region: Unit test
#[cfg(test)]
mod tests {
    use super::*;
    use tokenizers::TruncationDirection;
    use tokio::sync::OwnedSemaphorePermit;

    #[tokio::test]
    async fn test_infer_embedding_and_info() -> Result<()> {
        let (infer, info) = run(
            "./Qwen3-Embedding-0.6B".to_string(),
            None,
            Some(2),
            Some(DType::Float32),
            Some(Pool::Mean),
            2,
            512,
            Some(4),
            16,
            true,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            "text-embeddings-inference-test".to_string(),
        )
        .await
        .expect("Failed to create Infer and Info");
        let input = "This is a test".to_string();
        // Test embedding
        let permit = infer
            .try_acquire_permit()
            .map_err(|err| Error::Custom(err.to_string()))
            .unwrap();
        let embed_result = infer
            .embed_all(input, true, TruncationDirection::Left, None, permit)
            .await
            .map_err(|err| {
                error!("Embedding failed: {err}");
                err
            });
        println!("Embedding result: {:?}", embed_result);
        assert!(embed_result.is_ok());

        Ok(())
    }
}
// endregion: Unit test
