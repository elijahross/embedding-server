use crate::ai::ResponseMetadata;
use crate::ai::infer::{
    AllEmbeddingsInferResponse, Infer, InferMetadata, PooledEmbeddingsInferResponse,
};
use crate::ai::tokenization::{SimpleToken as CoreSimpleToken, into_tokens};
use crate::cache::AppState;
use crate::error::{Error, Result};
use crate::types::ErrorType;
use crate::types::{
    DecodeRequest, DecodeResponse, EmbedAllRequest, EmbedAllResponse, EmbedRequest, EmbedResponse,
    EmbedSparseRequest, EmbedSparseResponse, Embedding, EncodingFormat, ErrorResponse, Input,
    InputIds, InputType, OpenAICompatEmbedding, OpenAICompatErrorResponse, OpenAICompatRequest,
    OpenAICompatResponse, OpenAICompatUsage, PredictInput, PredictRequest, PredictResponse,
    Prediction, Rank, RerankRequest, RerankResponse, Sequence, SimilarityInput,
    SimilarityParameters, SimilarityRequest, SimilarityResponse, SimpleToken, SparseValue,
    TokenizeInput, TokenizeRequest, TokenizeResponse, TruncationDirection, VertexPrediction,
    VertexRequest, VertexResponse,
};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Extension},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::post,
};
use futures::future::join_all;
use lib_embedding::error::Error as TextEmbeddingsError;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::sync::OwnedSemaphorePermit;

pub fn serve_embed() -> Router {
    Router::new().route("/embed", post(run_embed))
}
use tracing::instrument;

#[utoipa::path(
post,
tag = "Text Embeddings Inference",
path = "/embed",
request_body = EmbedRequest,
responses(
(status = 200, description = "Embeddings", body = EmbedResponse),
(status = 424, description = "Embedding Error", body = ErrorResponse,
example = json ! ({"error": "Inference failed", "error_type": "backend"})),
(status = 429, description = "Model is overloaded", body = ErrorResponse,
example = json ! ({"error": "Model is overloaded", "error_type": "overloaded"})),
(status = 422, description = "Tokenization error", body = ErrorResponse,
example = json ! ({"error": "Tokenization error", "error_type": "tokenizer"})),
(status = 400, description = "Batch is empty", body = ErrorResponse,
example = json ! ({"error": "Batch is empty", "error_type": "empty"})),
(status = 413, description = "Batch size error", body = ErrorResponse,
example = json ! ({"error": "Batch size error", "error_type": "validation"})),
)
)]
#[instrument(
    skip_all,
    fields(total_time, tokenization_time, queue_time, inference_time,)
)]
async fn run_embed(
    Extension(app_state): Extension<AppState>,
    Json(req): Json<EmbedRequest>,
) -> Result<Response> {
    let infer = app_state.infer.clone();
    let info = app_state.info.clone();
    let span = tracing::Span::current();

    let start_time = Instant::now();
    let truncate = req.truncate.unwrap_or(info.auto_truncate);

    // Wrap everything in a Result so we can match errors
    let result: Result<(EmbedResponse, ResponseMetadata)> = (|| async {
        match req.inputs {
            Input::Single(input) => {
                metrics::counter!("te_request_count", "method" => "single").increment(1);
                let compute_chars = input.count_chars();

                let permit = infer
                    .try_acquire_permit()
                    .map_err(|err| Error::Custom(err.to_string()))?;
                let response = infer
                    .embed_pooled(
                        input,
                        truncate,
                        req.truncation_direction.into(),
                        req.prompt_name,
                        req.normalize,
                        req.dimensions,
                        permit,
                    )
                    .await?;

                metrics::counter!("te_request_success", "method" => "single").increment(1);

                Ok((
                    EmbedResponse(vec![response.results]),
                    ResponseMetadata::new(
                        compute_chars,
                        response.metadata.prompt_tokens,
                        start_time,
                        response.metadata.tokenization,
                        response.metadata.queue,
                        response.metadata.inference,
                    ),
                ))
            }
            Input::Batch(inputs) => {
                metrics::counter!("te_request_count", "method" => "batch").increment(1);

                if inputs.is_empty() {
                    return Err(Error::Custom("`inputs` cannot be empty".to_string()));
                }

                let batch_size = inputs.len();
                if batch_size > info.max_client_batch_size {
                    return Err(Error::Custom(format!(
                        "batch size {batch_size} > maximum allowed batch size {}",
                        info.max_client_batch_size
                    )));
                }

                let mut futures = Vec::with_capacity(batch_size);
                let mut compute_chars = 0;

                for input in inputs {
                    compute_chars += input.count_chars();
                    let local_infer = infer.clone();
                    let prompt_name = req.prompt_name.clone();
                    futures.push(async move {
                        let permit = local_infer.acquire_permit().await;
                        local_infer
                            .embed_pooled(
                                input,
                                truncate,
                                req.truncation_direction.into(),
                                prompt_name,
                                req.normalize,
                                req.dimensions,
                                permit,
                            )
                            .await
                    })
                }

                let results = futures::future::join_all(futures)
                    .await
                    .into_iter()
                    .collect::<Result<Vec<PooledEmbeddingsInferResponse>>>()?;

                let mut embeddings = Vec::with_capacity(batch_size);
                let mut total_tokenization_time = 0;
                let mut total_queue_time = 0;
                let mut total_inference_time = 0;
                let mut total_compute_tokens = 0;

                for r in results {
                    total_tokenization_time += r.metadata.tokenization.as_nanos() as u64;
                    total_queue_time += r.metadata.queue.as_nanos() as u64;
                    total_inference_time += r.metadata.inference.as_nanos() as u64;
                    total_compute_tokens += r.metadata.prompt_tokens;
                    embeddings.push(r.results);
                }

                let batch_size = batch_size as u64;
                metrics::counter!("te_request_success", "method" => "batch").increment(1);

                Ok((
                    EmbedResponse(embeddings),
                    ResponseMetadata::new(
                        compute_chars,
                        total_compute_tokens,
                        start_time,
                        Duration::from_nanos(total_tokenization_time / batch_size),
                        Duration::from_nanos(total_queue_time / batch_size),
                        Duration::from_nanos(total_inference_time / batch_size),
                    ),
                ))
            }
        }
    })()
    .await;

    match result {
        Ok((response, metadata)) => {
            metadata.record_span(&span);
            metadata.record_metrics();
            let headers = HeaderMap::from(metadata);
            tracing::info!("Success");
            Ok((headers, Json(response)).into_response())
        }

        Err(Error::Custom(msg)) if msg.contains("Queue is full") => {
            tracing::warn!("Queue full: returning 429");
            Ok((StatusCode::TOO_MANY_REQUESTS, Json(json!({ "error": msg }))).into_response())
        }

        Err(err) => {
            tracing::error!("Handler error: {err}");
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": err.to_string() })),
            )
                .into_response())
        }
    }
}
