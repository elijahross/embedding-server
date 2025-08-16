use crate::error::Error;
use crate::log::log_request;
use crate::middleware::mw_auth::Ctm;
use axum::http::{Method, Uri};
use axum::response::Response;
use tracing::info;
use uuid::Uuid;

/// This middleware function is used to map the response after a request
/// has been processed. It performs the following tasks:
///
/// 1. Logs information about the response using the `log_request` function.
/// 2. Extracts the context (`Ctm`) from the request extensions if available.
/// 3. Generates a new UUID for the request.
/// 4. Retrieves any potential error associated with the response from the
///    request extensions.
/// 5. Extracts the HTTP method from the request.
/// 6. Calls the `log_request` function with the extracted information.
/// 7. Returns the original response without modification.
pub async fn mw_response_map(res: Response) -> Response {
    info!("MIDDLEWARE: Logging Response");

    let uuid = Uuid::new_v4().to_string();
    let web_error = res.extensions().get::<Error>().cloned();
    let ctx = res.extensions().get::<Ctm>().map(|c| c.0.clone());
    let uri = res.extensions().get::<Uri>().cloned().unwrap_or_default();
    let http_method = res
        .extensions()
        .get::<Method>()
        .cloned()
        .unwrap_or(Method::GET);

    let _ = log_request(uuid, http_method, uri, ctx, web_error).await;

    res
}
