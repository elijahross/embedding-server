use crate::error::{Error, Result};
use crate::config::auth_config;
use lib_core::{
    database::ModelManager,
    model::files::{FileMac, FileForCreate, FileForUpdate, File},
};
use serde_with::chrono::NaiveDateTime;
use lib_storage::functions::files::{list_files_in_bucket, generate_presigned_url, delete_file};
use lib_storage::Client;
use lib_ai::Embeddings;

pub struct UserAppointmentChron {
    start_time: Option<NaiveDateTime>,
    end_time: Option<NaiveDateTime>,
    title: String,
    description: Option<String>,
}

pub struct DoclingResponse {
    pub content_md: String,
}

pub async fn process_new_files(mm: &ModelManager, client: &Client, embedder: &Embeddings) -> Result<()> {
    let new_files = FileMac::get_files_without_content(mm).await?;
    let config = auth_config();
    for file in new_files {
        let presigned_url = generate_presigned_url(client, &config.bucket, &file.filename, 600).await?;
        let request = reqwest::Client::new()
            .post(&config.parser)
            .json(&serde_json::json!({
                "url": presigned_url,
                "filename": file.filename,
            }))
            .send()
            .await?;

        let content_md: DoclingResponse = request.json().await?;
        let embedding = embedder.embed(&content_md.content_md).await?;
        let update = FileForUpdate {
            content_md: Some(content_md.content_md),
            embedding: Some(embedding),
            filename: None, // No need to update filename
        };
        FileMac::update_file(mm, &file.file_id, update).await?;
    }
    Ok(())
}

pub async fn sync_s3_files(mm: &ModelManager, client: &Client) -> Result<()> {
    let config = auth_config();
    let s3_files = list_files_in_bucket(client, &config.bucket).await?;
    let db_files = FileMac::get_all_files(mm).await?;

    for s3_file in s3_files {
        if !db_files.iter().any(|f| f.filename == s3_file) {
            let file = FileForCreate {
                file_id: Uuid::new_v4().to_string(),
                applicant: "default_applicant".to_string(),
                filename: s3_file.clone(),
                content_md: None,
                embedding: None,
            };
            FileMac::create_file(mm, file).await?;
        }
    }
    for db_file in db_files {
        if !s3_files.contains(&db_file.filename) {
            FileMac::delete_file(mm, &db_file.file_id).await?;
        }
    }
    Ok(())
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;
    use lib_core::database::ModelManager;

    #[tokio::test]
    async fn test_process_new_files() -> Result<()> {
        let mm = ModelManager::new().await?;
        let client = Client::new().await?;
        let embedder = Embeddings::new().await?;

        // Mock the parser URL in the environment
        std::env::set_var("PARSER_URL", "http://localhost:8000/parse");

        // Run the process_new_files function
        process_new_files(&mm, &client, &embedder).await?;

        // Verify that files were processed and updated correctly
        let files = FileMac::get_all_files(&mm).await?;
        assert!(!files.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_sync_s3_files() -> Result<()> {
        let mm = ModelManager::new().await?;
        let client = Client::new().await?;

        // Mock the S3 bucket contents
        std::env::set_var("S3_BUCKET", "uploaded-files");

        // Run the sync_s3_files function
        sync_s3_files(&mm, &client).await?;

        // Verify that files were synced correctly
        let files = FileMac::get_all_files(&mm).await?;
        assert!(!files.is_empty());

        Ok(())
    }
}
// endregion: Unit Test