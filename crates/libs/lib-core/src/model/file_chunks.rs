use crate::database::ModelManager;
use crate::error::Result;
use pgvector::Vector;
use serde::{Deserialize, Serialize};
use sqlx::types::chrono::NaiveDateTime;
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct FileChunk {
    pub chunk_id: i64,
    pub file_id: String,
    pub chunk_index: i32,
    pub content_md: Option<String>,
    pub embedding: Option<Vector>,
    pub token_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileChunkForCreate {
    pub file_id: String,
    pub chunk_index: i32,
    pub content_md: Option<String>,
    pub embedding: Option<Vector>,
    pub token_count: Option<i32>,
}
#[derive(Debug, Deserialize, Clone)]
pub struct FileChunkForUpdate {
    pub chunk_index: Option<i32>,
    pub content_md: Option<String>,
    pub embedding: Option<Vector>,
    pub token_count: Option<i32>,
}

pub struct FileChunkMac;

impl FileChunkMac {
    pub async fn create_chunk(mm: &ModelManager, chunk: FileChunkForCreate) -> Result<FileChunk> {
        let db = mm.db();
        let query = sqlx::query_as::<_, FileChunk>(
            r#"
            INSERT INTO file_chunks (file_id, chunk_index, content_md, embedding, token_count)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(chunk.file_id)
        .bind(chunk.chunk_index)
        .bind(chunk.content_md)
        .bind(chunk.embedding.map(Vector::from))
        .bind(chunk.token_count);

        let chunk = query.fetch_one(db).await?;
        Ok(chunk)
    }

    pub async fn get_chunk_by_id(mm: &ModelManager, chunk_id: i64) -> Result<FileChunk> {
        let db = mm.db();
        let query = sqlx::query_as::<_, FileChunk>(
            r#"
            SELECT * FROM file_chunks WHERE chunk_id = $1
            "#,
        )
        .bind(chunk_id);

        let chunk = query.fetch_one(db).await?;
        Ok(chunk)
    }

    pub async fn update_chunk(mm: &ModelManager, chunk_id: i64, update: FileChunkForUpdate) -> Result<FileChunk> {
        let db = mm.db();
        let query = sqlx::query_as::<_, FileChunk>(
            r#"
            UPDATE file_chunks
            SET
                chunk_index = COALESCE($2, chunk_index),
                content_md = COALESCE($3, content_md),
                embedding = COALESCE($4, embedding),
                token_count = COALESCE($5, token_count)
            WHERE chunk_id = $1
            RETURNING *
            "#,
        )
        .bind(chunk_id)
        .bind(update.chunk_index)
        .bind(update.content_md)
        .bind(update.embedding.map(Vector::from))
        .bind(update.token_count);

        let chunk = query.fetch_one(db).await?;
        Ok(chunk)
    }

    pub async fn delete_chunk(mm: &ModelManager, chunk_id: i64) -> Result<u64> {
        let res = sqlx::query(
            r#"
            DELETE FROM file_chunks WHERE chunk_id = $1
            "#,
        )
        .bind(chunk_id)
        .execute(mm.db())
        .await?;

        Ok(res.rows_affected())
    }

    pub async fn get_chunks_by_file_id(mm: &ModelManager, file_id: &str) -> Result<Vec<FileChunk>> {
        let db = mm.db();
        let chunks = sqlx::query_as::<_, FileChunk>(
            r#"
            SELECT * FROM file_chunks WHERE file_id = $1
            ORDER BY chunk_index
            "#,
        )
        .bind(file_id)
        .fetch_all(db)
        .await?;

        Ok(chunks)
    }

    pub async fn get_chunks_without_embedding(mm: &ModelManager) -> Result<Vec<FileChunk>> {
        let db = mm.db();
        let chunks = sqlx::query_as::<_, FileChunk>(
            r#"
            SELECT * FROM file_chunks WHERE embedding IS NULL
            "#,
        )
        .fetch_all(db)
        .await?;

        Ok(chunks)
    }

    pub async fn search_chunks_by_keyword(mm: &ModelManager, file_id: &str, keyword: &str) -> Result<Vec<FileChunk>> {
        let db = mm.db();
        let pattern = format!("%{}%", keyword);
        let chunks = sqlx::query_as::<_, FileChunk>(
            r#"
            SELECT * FROM file_chunks
            WHERE file_id = $1
            AND content_md ILIKE $2
            "#,
        )
        .bind(file_id)
        .bind(pattern)
        .fetch_all(db)
        .await?;
        Ok(chunks)
    }

    pub async fn search_chunks_by_embedding(mm: &ModelManager, file_id: &str, embedding: Vec<f32>, limit: i64) -> Result<Vec<FileChunk>> {
        let db = mm.db();
        let chunks = sqlx::query_as::<_, FileChunk>(
            r#"
            SELECT *
            FROM file_chunks
            WHERE file_id = $1
            AND embedding IS NOT NULL
            ORDER BY embedding <-> $2
            LIMIT $3
            "#,
        )
        .bind(file_id)
        .bind(Vector::from(embedding))
        .bind(limit)
        .fetch_all(db)
        .await?;
        Ok(chunks)
    }
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::ModelManager;
    use pgvector::Vector;

    async fn setup_mm() -> ModelManager {
        // Replace with your test DB URL
        ModelManager::new("postgres://user:pass@localhost/test_db").await.unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get_chunk() {
        let mm = setup_mm().await;

        // Create
        let chunk_in = FileChunkForCreate {
            file_id: "test_file".into(),
            chunk_index: 0,
            content_md: Some("Hello world".into()),
            embedding: Some(Vector::from(vec![0.1, 0.2, 0.3])),
            token_count: Some(3),
        };
        let chunk = FileChunkMac::create_chunk(&mm, chunk_in.clone()).await.unwrap();
        assert_eq!(chunk.file_id, "test_file");
        assert_eq!(chunk.chunk_index, 0);

        // Get by ID
        let fetched = FileChunkMac::get_chunk_by_id(&mm, chunk.chunk_id).await.unwrap();
        assert_eq!(fetched.chunk_id, chunk.chunk_id);
        assert_eq!(fetched.content_md, chunk.content_md);
    }

    #[tokio::test]
    async fn test_update_chunk() {
        let mm = setup_mm().await;

        let chunk_in = FileChunkForCreate {
            file_id: "update_test".into(),
            chunk_index: 1,
            content_md: Some("Original".into()),
            embedding: Some(Vector::from(vec![0.1, 0.1])),
            token_count: Some(2),
        };
        let chunk = FileChunkMac::create_chunk(&mm, chunk_in).await.unwrap();

        let update = FileChunkForUpdate {
            chunk_index: Some(2),
            content_md: Some("Updated".into()),
            embedding: None,
            token_count: Some(4),
        };

        let updated = FileChunkMac::update_chunk(&mm, chunk.chunk_id, update).await.unwrap();
        assert_eq!(updated.chunk_index, 2);
        assert_eq!(updated.content_md.unwrap(), "Updated");
        assert_eq!(updated.token_count.unwrap(), 4);
    }

    #[tokio::test]
    async fn test_delete_chunk() {
        let mm = setup_mm().await;

        let chunk_in = FileChunkForCreate {
            file_id: "delete_test".into(),
            chunk_index: 0,
            content_md: Some("Delete me".into()),
            embedding: None,
            token_count: Some(2),
        };
        let chunk = FileChunkMac::create_chunk(&mm, chunk_in).await.unwrap();

        let deleted_rows = FileChunkMac::delete_chunk(&mm, chunk.chunk_id).await.unwrap();
        assert_eq!(deleted_rows, 1);

        // Should fail to fetch
        let result = FileChunkMac::get_chunk_by_id(&mm, chunk.chunk_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_chunks_by_keyword() {
        let mm = setup_mm().await;

        let chunk_in = FileChunkForCreate {
            file_id: "keyword_test".into(),
            chunk_index: 0,
            content_md: Some("Searchable content".into()),
            embedding: None,
            token_count: Some(2),
        };
        let _ = FileChunkMac::create_chunk(&mm, chunk_in).await.unwrap();

        let results = FileChunkMac::search_chunks_by_keyword(&mm, "keyword_test", "Searchable")
            .await
            .unwrap();
        assert!(!results.is_empty());
    }
}
// endregion: Unit Test