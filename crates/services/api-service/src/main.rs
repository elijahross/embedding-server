mod cache;
pub mod config;
pub mod error;
mod log;
mod middleware;
mod routes;

pub use self::error::{Error, Result};
use crate::cache::AppState;
use crate::middleware::mw_auth::{UserToken, ctx_resolver, request_auth};
use crate::middleware::mw_response::mw_response_map;
use axum::{Router, extract::Extension, serve};
use candle_core::Device;
use lib_ai::Embeddings;
use lib_core::database::ModelManager;
use lib_storage::create_aws_client;
use moka::future::Cache;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::time::Duration;
use tower_cookies::CookieManagerLayer;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};

use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("Initializing Environment");
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(&addr).await.unwrap();

    let mm = ModelManager::new().await?;
    let app_state = AppState::new(Arc::new(mm)).await?;

    // Rate limiting Configuration
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
            if !governor_limiter.is_empty() {
                tracing::info!("Rate limiting storage size: {}", governor_limiter.len());
            }
            governor_limiter.retain_recent();
        }
    });

    let routes_api = Router::new()
        .merge(routes::cron::serve_cron(mm.clone()))
        .route_layer(middleware::from_fn(request_auth))
        .layer(GovernorLayer {
            config: governor_conf,
        });

    let global_routes = Router::new()
        .nest("/api/v1", routes_api)
        .layer(axum::middleware::from_fn_with_state(
            mm.clone(),
            ctx_resolver,
        ))
        .layer(axum::middleware::map_response(mw_response_map))
        .layer(CookieManagerLayer::new())
        .layer(Extension(app_state.clone()));

    info!("Server started on: http://{}", addr);
    serve(listener, global_routes.into_make_service())
        .await
        .unwrap();

    Ok(())
}
