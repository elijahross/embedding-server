use crate::error::{Error, Result};
use lib_cron::ChronJobs;
use axum::{
    extract::Extension,
    response::{IntoResponse, Json, Response},
    routing::post,
    Router,
};
use serde_json::json;

pub fn serve_api() -> Router {
    Router::new()
        .route("/add", post(add_chron_job))
        .route("/delete", post(delete_chron_job))
        .route("/restart", post(restart_chron_jobs).get(get_informaiton))
}

async fn add_chron_job(
    Extension(cron_jobs): Extension<ChronJobs>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response> {
    let res;
    if let Some(data) = payload.get("data") {
        let cron = data
            .get("cron")
            .and_then(|c| c.as_str())
            .ok_or(Error::ChronFails("Missing data".to_string()))?;
        let description = data
            .get("description")
            .and_then(|c| c.as_str())
            .ok_or(Error::ChronFails("Missing data".to_string()))?;
        let _ = cron_jobs
            .add_job(description.to_string(), cron.to_string())
            .await;
        res = json!({
            "status": 200,
            "data": "ok",
        });
    } else {
        res = json!({
            "status": 401,
            "error": "Missing require data",
        });
    }
    Ok(Json(res).into_response())
}

async fn delete_chron_job(
    Extension(cron_jobs): Extension<ChronJobs>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response> {
    let res;
    if let Some(data) = payload.get("data") {
        let id = data
            .get("id")
            .and_then(|c| c.as_str())
            .ok_or(Error::ChronFails("Missing data".to_string()))?;
        let uuid =
            uuid::Uuid::parse_str(id).map_err(|_| Error::ChronFails("Invalid UUID".to_string()))?;
        let _ = cron_jobs.remove_job(uuid).await;
        res = json!({
            "status": 200,
            "data": "ok",
        });
    } else {
        res = json!({
            "status": 401,
            "error": "Missing require data",
        });
    }
    Ok(Json(res).into_response())
}

async fn restart_chron_jobs(Extension(_cron_jobs): Extension<ChronJobs>) -> Result<Response> {
    let res = json!({
        "status": 200,
        "message": "ok"
    });
    Ok(Json(res).into_response())
}

async fn get_informaiton(Extension(cron_jobs): Extension<ChronJobs>) -> Result<Response> {
    let jobs = cron_jobs.cache.get_jobs().await;

    let res = json!({
        "status": 200,
        "data": jobs
    });
    Ok(Json(res).into_response())
}