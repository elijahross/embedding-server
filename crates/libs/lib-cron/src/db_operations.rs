use crate::error::{Error, Result};
use crate::config::auth_config;
use lib_core::{
    database::ModelManager,
    model::files::{FileMac, FileForCreate, FileForUpdate, File},
    model::file_chunks::{FileChunkMac, FileChunkForCreate, FileChunkForUpdate},
};
use serde_with::chrono::NaiveDateTime;
use lib_storage::functions::files::{list_files_in_bucket, generate_presigned_url, delete_file};
use lib_storage::Client;
use lib_ai::Embeddings;

pub struct DoclingResponse {
    pub content_md: String,
}

pub async fn process_new_files(
    mm: &ModelManager,
    storage: &StorageClient,
    embedder: &Embeddings,
) -> Result<()> {
    let config = auth_config();
    let http = reqwest::Client::builder()
        .pool_idle_timeout(Some(Duration::from_secs(30)))
        .build()
        .map_err(|e| Error::Custom(format!("http client build failed: {e}")))?;

    // Fetch files that need processing (metadata exists but no chunks yet)
    let new_files = FileMac::get_unprocessed_files(mm).await?;

    for file in new_files {
        // ===== 1) Fetch parsed markdown from your parser (with signed URL + retries) =====
        let presigned_url = generate_presigned_url(storage, &config.bucket, &file.filename, 600)
            .await
            .map_err(|e| Error::Custom(format!("presign url failed for {}: {e}", file.filename)))?;

        let content_md = fetch_markdown_with_retry(
            &http,
            &config.parser,
            &file.filename,
            &presigned_url,
            3,                       // retries
            Duration::from_millis(400),
        ).await?;

        // ===== 2) Split → Semantic compression (token-safe) =====
        let max_tokens = 512; // match BERT config (max_position_embeddings)
        let raw_chunks = split_markdown_into_chunks(embedder, &content_md.content_md, max_tokens);
        if raw_chunks.is_empty() {
            // Optionally: log
            continue;
        }

        // Higher threshold = fewer merges (more chunks kept)
        let threshold = 0.85_f32;
        let semantic_chunks = semantic_compression(embedder, raw_chunks, threshold).await?;

        // Insert semantic chunks
        // NOTE: prefer using your DAO (FileMac::insert_file_chunk_tx) which takes &mut tx.
        for (i, (chunk_text, embedding)) in semantic_chunks.into_iter().enumerate() {
            let token_count = embedder
                .tokenizer
                .encode(chunk_text.as_str(), true)
                .map_err(|e| Error::Custom(format!("tokenize merged chunk failed: {e}")))?
                .len();

            let chunk = FileChunkForCreate {
                file_id: file.file_id.clone(),
                chunk_index: i as i32,
                content_md: chunk_text,
                embedding: Some(embedding),
                token_count: token_count as i32,
            };

            FileChunkMac::create_chunk(
                mm,
                chunk
            ).await?;
        }
        // Mark file as processed
        let file_update = FileForUpdate {
            filename: Some(file.filename.clone()),
            proccessed: Some(true),
        };
        FileMac::update_file(mm, &file.file_id, file_update).await?;
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
) -> Result<DoclingResponse> {
    let body = json!({
        "url": presigned_url,
        "filename": filename,
    });

    let mut attempt = 0usize;
    loop {
        attempt += 1;
        let resp = http
            .post(parser_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Custom(format!("parser request failed (attempt {attempt}): {e}")))?;

        if resp.status().is_success() {
            let parsed = resp
                .json::<DoclingResponse>()
                .await
                .map_err(|e| Error::Custom(format!("parser json decode failed: {e}")))?;
            return Ok(parsed);
        }

        if attempt >= max_retries {
            return Err(Error::Custom(format!(
                "parser returned status {} after {attempt} attempts",
                resp.status()
            )));
        }

        let jitter = Duration::from_millis(fastrand::u64(0..100));
        let wait = base_backoff * (1u32 << (attempt - 1)) + jitter;
        sleep(wait).await;
    }
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
                filetype: s3._file.split('.').last().unwrap_or("unknown").to_string(),
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

// Test data beeing processing consistent with the original file
fn split_markdown_into_chunks(embedder: &Embeddings, content: &str, max_tokens: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for paragraph in content.split("\n\n") {
        let test_chunk = if current_chunk.is_empty() {
            paragraph.to_string()
        } else {
            format!("{}\n\n{}", current_chunk, paragraph)
        };

        let token_count = embedder.tokenizer.encode(test_chunk.clone(), true)
            .unwrap()
            .len();

        if token_count > max_tokens {
            if !current_chunk.is_empty() {
                chunks.push(current_chunk.clone());
                current_chunk.clear();
            }
            // If a single paragraph is too large, hard split it
            let mut sentence_chunk = String::new();
            for sentence in paragraph.split('.') {
                let test_sentence_chunk = format!("{}. ", sentence_chunk.clone() + sentence);
                let sentence_tokens = embedder.tokenizer.encode(test_sentence_chunk.clone(), true)
                    .unwrap()
                    .len();
                if sentence_tokens > max_tokens {
                    chunks.push(sentence_chunk.clone());
                    sentence_chunk.clear();
                }
                sentence_chunk.push_str(sentence);
                sentence_chunk.push('.');
            }
            if !sentence_chunk.is_empty() {
                chunks.push(sentence_chunk);
            }
        } else {
            current_chunk = test_chunk;
        }
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

async fn semantic_compression(embedder: &Embeddings, chunks: Vec<String>, threshold: f32) -> Result<Vec<(String, Vec<f32>)>> {
    let chunk_refs: Vec<&str> = chunks.iter().map(|s| s.as_str()).collect();
    let embeddings = embedder.embed(&chunk_refs)?;

    let mut used = vec![false; chunks.len()];
    let mut semantic_chunks = Vec::new();

    for i in 0..chunks.len() {
        if used[i] { continue; }
        let mut merged_text = chunks[i].clone();
        let mut merged_vecs = vec![embeddings[i].clone()];
        used[i] = true;

        for j in (i+1)..chunks.len() {
            if used[j] { continue; }
            let sim = embedder.similarity(&[embeddings[i].clone()], &[embeddings[j].clone()])[0][0];
            if sim > threshold {
                merged_text.push_str("\n\n");
                merged_text.push_str(&chunks[j]);
                merged_vecs.push(embeddings[j].clone());
                used[j] = true;
            }
        }

        let merged_emb = average_embeddings(&merged_vecs);
        semantic_chunks.push((merged_text, merged_emb));
    }

    Ok(semantic_chunks)
}

fn average_embeddings(embs: &[Vec<f32>]) -> Vec<f32> {
    let dim = embs[0].len();
    let mut sum = vec![0.0; dim];
    for e in embs {
        for (i, v) in e.iter().enumerate() {
            sum[i] += v;
        }
    }
    sum.iter_mut().for_each(|v| *v /= embs.len() as f32);
    sum
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