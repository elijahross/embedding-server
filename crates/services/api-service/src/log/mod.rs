use crate::error::{Error, Result};
use axum::http::{Method, Uri};
use chrono::prelude::*;
use lib_core::ctx::Ctx;
use serde::Serialize;
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Serialize)]
struct RequestLogLine {
    uuid: String,
    timestamp: String,

    user_id: Option<String>,
    http_method: String,
    http_path: String,

    error: Option<String>,
}

pub async fn log_request(
    uuid: String,
    http_method: Method,
    uri: Uri,
    ctx: Option<Ctx>,
    web_error: Option<Error>,
) -> Result<()> {
    let user_id = ctx.map(|c| c.user_id());
    let http_method = http_method.to_string();
    let http_path = uri.path().to_string();

    let error = web_error.as_ref().map(|e| format!("{:?}", e));

    let log_line = RequestLogLine {
        uuid,
        timestamp: Utc::now().to_rfc3339(),
        user_id,
        http_method,
        http_path,
        error,
    };

    //Implement: send LogLine to cloudwatch..

    let log_lines = serde_json::to_string(&log_line)?;
    println!("{}", log_lines);
    Ok(())
}