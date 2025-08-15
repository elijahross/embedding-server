use crate::error::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::chrono::NaiveDateTime;
use sqlx::FromRow;
use crate::database::ModelManager;

// region: Structs

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct File {
    pub file_id: String,
    pub applicant: String,
    pub filename: String,
    pub file_type: String,
    pub created_at: NaiveDateTime,
    pub proccessed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileForCreate {
    pub applicant: String,
    pub filename: String,
    pub file_type: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FileForUpdate {
    pub filename: Option<String>,
    pub proccessed: Option<bool>,
}

// endregion: Structs

// region: CRUD + Search

pub struct FileMac;

impl FileMac {
    pub async fn create_file(mm: &ModelManager, file: FileForCreate) -> Result<File> {
        let db = mm.db();
        let query = sqlx::query_as::<_, File>(
            r#"
            INSERT INTO files (applicant, filename, file_type)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(file.applicant)
        .bind(file.filename)
        .bind(file.file_type);

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

    pub fn get_unprocessed_files(mm: &ModelManager) -> Result<Vec<File>> {
        let db = mm.db();
        let files = sqlx::query_as::<_, File>(
            r#"
            SELECT * FROM files WHERE proccessed = FALSE
            "#,
        )
        .fetch_all(db)
        .await?;

        Ok(files)
    }

    pub async fn update_file(mm: &ModelManager, file_id: &str, update: FileForUpdate) -> Result<File> {
        let db = mm.db();
        let query = sqlx::query_as::<_, File>(
            r#"
            UPDATE files
            SET
                filename = COALESCE($2, filename),
                proccessed = COALESCE($3, proccessed)
            WHERE file_id = $1
            RETURNING *
            "#,
        )
        .bind(file_id)
        .bind(update.filename)
        .bind(update.proccessed);

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

    pub async fn delete_files_by_applicant(mm: &ModelManager, applicant: &str) -> Result<u64> {
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
            applicant: "applicant_123".to_string(),
            filename: "example.pdf".to_string(),
        };

        let created_file = FileMac::create_file(&mm, new_file.clone()).await?;
        assert_eq!(created_file.applicant, new_file.applicant);
        assert_eq!(created_file.filename, new_file.filename);

        // Update filename and proccessed
        let update = FileForUpdate {
            filename: Some("updated_example.pdf".to_string()),
            proccessed: Some(true),
        };
        let updated_file = FileMac::update_file(&mm, &created_file.file_id, update).await?;
        assert_eq!(updated_file.filename, "updated_example.pdf");
        assert!(updated_file.proccessed);

        // Delete
        let deleted = FileMac::delete_file(&mm, &created_file.file_id).await?;
        assert_eq!(deleted, 1);

        Ok(())
    }
}

// endregion: Unit Test
