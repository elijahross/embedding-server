use crate::config::auth_config;
use crate::error::{Error, Result};
use aws_sdk_s3::Client;
use lib_core::{
    database::ModelManager,
    model::file_chunks::{FileChunkForCreate, FileChunkMac},
    model::files::{FileForCreate, FileForUpdate, FileMac},
};
use lib_storage::functions::file::{generate_presigned_url, list_files_in_bucket};
use serde::Deserialize;
use serde_json::json;
use tokio::time::{Duration, sleep};
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct DoclingResponse {
    pub document: Document,
    pub status: String,
    pub errors: Vec<String>,
    pub processing_time: f64,
    pub timings: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct Document {
    pub filename: String,
    pub md_content: String,
    pub json_content: Option<String>,
    pub html_content: Option<String>,
    pub text_content: Option<String>,
    pub doctags_content: Option<String>,
}

pub async fn process_new_files(mm: &ModelManager, storage: &Client) -> Result<()> {
    let config = auth_config();
    let http = reqwest::Client::builder()
        .pool_idle_timeout(Some(Duration::from_secs(30)))
        .build()
        .map_err(|e| Error::Custom(format!("http client build failed: {e}")))?;

    let new_files = FileMac::get_unprocessed_files(mm)
        .await
        .map_err(|e| Error::Custom(format!("failed to get unprocessed files: {}", e)))?;

    for file in new_files {
        let presigned_url = generate_presigned_url(storage, &config.bucket, &file.filename, 600)
            .await
            .map_err(|e| Error::Custom(format!("presign url failed for {}: {e}", file.filename)))?;

        let content_md = fetch_markdown_with_retry(
            &http,
            &config.parser,
            &file.filename,
            &presigned_url,
            3,
            Duration::from_millis(400),
        )
        .await?;
        // Extract text_content and filter out image markdown like [Image](data:image/png;base64,...)
        let mut text_content = content_md.text_content.unwrap_or_default();
        // Remove image markdown patterns
        let image_pattern = regex::Regex::new(r"\[Image\]\(data:image/[^)]+\)").unwrap();
        text_content = image_pattern.replace_all(&text_content, "").to_string();

        let max_tokens = auth_config().max_tokens as usize;
        /*
        let threshold = 0.85_f32;
        let semantic_chunks =
            semantic_compression(embedder, raw_chunks, threshold, max_tokens).await?; // Can be implemented if enougth ram is there
         */

        let file_update = FileForUpdate {
            filename: Some(file.filename.clone()),
            processed: Some(true),
        };
        FileMac::update_file(mm, &file.file_id, file_update)
            .await
            .map_err(|e| {
                Error::Custom(format!(
                    "failed to update file {} as processed: {}",
                    file.filename, e
                ))
            })?;
    }

    Ok(())
}

async fn fetch_markdown_with_retry(
    http: &reqwest::Client,
    parser_url: &str,
    filename: &str,
    presigned_url: &str,
    max_retries: usize,
    base_backoff: Duration,
) -> Result<Document> {
    let body = json!({"http_sources":[{
        "url": presigned_url,
        "filename": filename,
    }]});
    info!("Requesting parser at {} with body: {:?}", parser_url, body);
    let mut attempt = 0usize;
    loop {
        attempt += 1;
        let resp = http
            .post(parser_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                Error::Custom(format!("parser request failed (attempt {attempt}): {e}"))
            })?;

        if resp.status().is_success() {
            let parsed = resp
                .json::<DoclingResponse>()
                .await
                .map_err(|e| Error::Custom(format!("parser json decode failed: {e}")))?;
            return Ok(parsed.document);
        }

        if attempt >= max_retries {
            return Err(Error::Custom(format!(
                "parser returned status {} after {attempt} attempts with body: {:?}",
                resp.status(),
                body
            )));
        }

        let jitter = Duration::from_millis(fastrand::u64(0..100));
        let wait = base_backoff * (1u32 << (attempt - 1)) + jitter;
        sleep(wait).await;
    }
}

pub async fn sync_s3_files(mm: &ModelManager, client: &Client) -> Result<()> {
    let config = auth_config();
    let s3_files = list_files_in_bucket(client, &config.bucket, None)
        .await
        .map_err(|e| {
            Error::Custom(format!(
                "failed to list files in S3 bucket {}: {}",
                config.bucket, e
            ))
        })?;
    let db_files = FileMac::get_all_files(mm)
        .await
        .map_err(|e| Error::Custom(format!("failed to get all files from DB: {}", e)))?;

    for s3_file in s3_files.clone() {
        if !db_files.iter().any(|f| f.filename == s3_file) {
            let file = FileForCreate {
                applicant: "default_applicant".to_string(),
                filename: s3_file.clone(),
                file_type: s3_file.split('.').last().unwrap_or("unknown").to_string(),
            };
            FileMac::create_file(mm, file).await.map_err(|e| {
                Error::Custom(format!("failed to create file {} in DB: {}", s3_file, e))
            })?;
        }
    }
    for db_file in db_files {
        if !s3_files.contains(&db_file.filename) {
            FileMac::delete_file(mm, &db_file.file_id)
                .await
                .map_err(|e| {
                    Error::Custom(format!(
                        "failed to delete file {} from DB: {}",
                        db_file.filename, e
                    ))
                })?;
        }
    }
    Ok(())
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;
    use lib_core::_dev_utils::init_dev;
    use lib_core::database::ModelManager;
    use lib_storage::create_aws_client;

    #[tokio::test]
    async fn test_process_new_files() -> Result<()> {
        let db = init_dev()
            .await
            .map_err(|e| Error::Custom(format!("Failed to initialize dev database: {}", e)))?;
        let mm = ModelManager::dev(db);
        let client = create_aws_client().await;
        let device = Device::Cpu;
        let model_id = "intfloat/multilingual-e5-base";

        // Run the sync_s3_files function
        sync_s3_files(&mm, &client).await?;
        // Verify that files were processed and updated correctly
        let files = FileMac::get_all_files(&mm)
            .await
            .map_err(|e| Error::Custom(format!("Failed to get all files: {}", e)))?;
        assert!(!files.is_empty());

        // Run the process_new_files function
        process_new_files(&mm, &client).await?;

        // Verify that files were processed and updated correctly
        let file_chunks = FileChunkMac::search_chunks_by_keyword(&mm, "data", 10)
            .await
            .map_err(|e| Error::Custom(format!("Failed to get all file chunks: {}", e)))?;
        assert!(!file_chunks.is_empty());
        println!("File chunks: {:?}", file_chunks);

        Ok(())
    }
}
// endregion: Unit Test
