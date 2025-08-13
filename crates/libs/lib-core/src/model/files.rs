use crate::error::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::chrono::NaiveDateTime;
use sqlx::FromRow;
use crate::database::ModelManager;
use pgvector::Vector;

// region: Structs

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct File {
    pub file_id: String,
    pub applicant: String, // New field
    pub filename: String,
    pub content_md: Option<String>,
    pub embedding: Option<Vector>, // Optional pgvector
    pub uploaded_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileForCreate {
    pub applicant: String,
    pub filename: String,
    pub content_md: Option<String>,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FileForUpdate {
    pub filename: Option<String>,
    pub content_md: Option<String>,
    pub embedding: Option<Vec<f32>>,
}

// endregion: Structs

// region: CRUD + Search

pub struct FileMac;

impl FileMac {
    pub async fn create_file(mm: &ModelManager, file: FileForCreate) -> Result<File> {
        let db = mm.db();
        let query = sqlx::query_as::<_, File>(
            r#"
            INSERT INTO files (applicant, filename, content_md, embedding)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(file.applicant)
        .bind(file.filename)
        .bind(file.content_md)
        .bind(file.embedding.map(Vector::from));

        let file = query.fetch_one(db).await?;
        Ok(file)
    }

    pub async fn get_file_by_id(mm: &ModelManager, file_id: &str) -> Result<File> {
        let db = mm.db();
        let query = sqlx::query_as::<_, File>(
            r#"
            SELECT * FROM files WHERE file_id = $1
            "#,
        )
        .bind(file_id);

        let file = query.fetch_one(db).await?;
        Ok(file)
    }

    pub async fn update_file(mm: &ModelManager, file_id: &str, update: FileForUpdate) -> Result<File> {
        let db = mm.db();
        let query = sqlx::query_as::<_, File>(
            r#"
            UPDATE files
            SET
                filename = COALESCE($2, filename),
                content_md = COALESCE($3, content_md),
                embedding = COALESCE($4, embedding)
            WHERE file_id = $1
            RETURNING *
            "#,
        )
        .bind(file_id)
        .bind(update.filename)
        .bind(update.content_md)
        .bind(update.embedding.map(Vector::from));

        let file = query.fetch_one(db).await?;
        Ok(file)
    }

    pub async fn delete_file(mm: &ModelManager, file_id: &str) -> Result<u64> {
        let res = sqlx::query(
            r#"
            DELETE FROM files WHERE file_id = $1
            "#,
        )
        .bind(file_id)
        .execute(mm.db())
        .await?;

        Ok(res.rows_affected())
    }

    pub async fn get_files_without_content(mm: &ModelManager) -> Result<Vec<File>> {
        let db = mm.db();
        let files = sqlx::query_as::<_, File>(
            r#"
            SELECT * FROM files WHERE content_md IS NULL OR embedding IS NULL
            "#,
        )
        .fetch_all(db)
        .await?;

        Ok(files)
    }

    pub async fn delete_file_by_applicant(mm: &ModelManager, applicant: &str) -> Result<u64> {
        let res = sqlx::query(
            r#"
            DELETE FROM files WHERE applicant = $1
            "#,
        )
        .bind(applicant)
        .execute(mm.db())
        .await?;

        Ok(res.rows_affected())
    }

    pub async fn get_all_files(mm: &ModelManager) -> Result<Vec<File>> {
        let db = mm.db();
        let files = sqlx::query_as::<_, File>(
            r#"
            SELECT * FROM files
            "#,
        )
        .fetch_all(db)
        .await?;

        Ok(files)
    }

    pub async fn search_by_keyword(mm: &ModelManager, applicant: &str, keyword: &str) -> Result<Vec<File>> {
        let db = mm.db();
        let pattern = format!("%{}%", keyword);
        let files = sqlx::query_as::<_, File>(
            r#"
            SELECT * FROM files
            WHERE applicant = $1
            AND (filename ILIKE $2 OR content_md ILIKE $2)
            "#,
        )
        .bind(applicant)
        .bind(pattern)
        .fetch_all(db)
        .await?;
        Ok(files)
    }

    pub async fn search_by_embedding(mm: &ModelManager, applicant: &str, embedding: Vec<f32>, limit: i64) -> Result<Vec<File>> {
        let db = mm.db();
        let files = sqlx::query_as::<_, File>(
            r#"
            SELECT *
            FROM files
            WHERE applicant = $1
            AND embedding IS NOT NULL
            ORDER BY embedding <-> $2
            LIMIT $3
            "#,
        )
        .bind(applicant)
        .bind(Vector::from(embedding))
        .bind(limit)
        .fetch_all(db)
        .await?;
        Ok(files)
    }
}

// endregion: CRUD + Search

// region: Unit Test

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::ModelManager;
    use crate::error::Result;

    #[tokio::test]
    async fn test_file_mac() -> Result<()> {
        let mm = ModelManager::new().await?;

        let new_file = FileForCreate {
            file_id: "test_file".to_string(),
            applicant: "applicant_123".to_string(),
            filename: "example.pdf".to_string(),
            content_md: None,
            embedding: None,
        };

        let created_file = FileMac::create_file(&mm, new_file.clone()).await?;
        assert_eq!(created_file.file_id, new_file.file_id);
        assert_eq!(created_file.applicant, new_file.applicant);
        assert!(created_file.content_md.is_none());
        assert!(created_file.embedding.is_none());

        Ok(())
    }
}

// endregion: Unit Test