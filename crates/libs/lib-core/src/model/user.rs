use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sqlx::types::chrono::NaiveDateTime;
use sqlx::types::Uuid;
use sqlx::FromRow;

use crate::database::ModelManager;

// region:  Structs

#[derive(sqlx::Type, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[sqlx(type_name = "role")]
#[sqlx(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Viewer,
    Inactive,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin => write!(f, "Admin"),
            Role::Viewer => write!(f, "Viewer"),
            Role::Inactive => write!(f, "Inactive"),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct User {
    pub user_id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub role: Role,
    #[serde_as(as = "serde_with::NoneAsEmptyString")]
    pub api_key: Option<String>,
    #[serde_as(as = "chrono::DateTime<chrono::Utc>")]
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserForCreate {
    pub user_id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UserForUpdate {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub role: Option<Role>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserForAuthentication {
    pub user_id: String,
    pub salt: Uuid,
    pub api_key: Option<String>,
    pub role: Role,
}

// endregion:  Structs

// region: CRUD
pub struct UserBmc;
impl UserBmc {
    pub async fn create_user(mm: &ModelManager, user: UserForCreate) -> Result<User> {
        let db = mm.db();
        let query = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (user_id, first_name, last_name, email, role)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(user.user_id)
        .bind(user.first_name)
        .bind(user.last_name)
        .bind(user.email)
        .bind(Role::Viewer);

        let user = query.fetch_one(db).await?;
        Ok(user)
    }

    pub async fn get_user_by_id(mm: &ModelManager, user_id: &str) -> Result<User> {
        let db = mm.db();
        let query = sqlx::query_as::<_, User>(
            r#"
            SELECT * FROM users WHERE user_id = $1
            "#,
        )
        .bind(user_id);

        let user = query.fetch_one(db).await?;
        Ok(user)
    }

    pub async fn get_user_by_email(mm: &ModelManager, email: &str) -> Result<User> {
        let db = mm.db();
        let query = sqlx::query_as::<_, User>(
            r#"
            SELECT * FROM users WHERE email = $1
            "#,
        )
        .bind(email);

        let user = query.fetch_one(db).await?;
        Ok(user)
    }

    pub async fn get_user_by_api_key(mm: &ModelManager, api_key: &str) -> Result<User> {
        let db = mm.db();
        let query = sqlx::query_as::<_, User>(
            r#"
            SELECT * FROM users WHERE api_key = $1
            "#,
        )
        .bind(api_key);

        let user = query.fetch_one(db).await?;
        Ok(user)
    }

    pub async fn update_user(
        mm: &ModelManager,
        user_id: &str,
        user_update: UserForUpdate,
    ) -> Result<User> {
        let db = mm.db();
        let query = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET
                first_name = COALESCE($2, first_name),
                last_name = COALESCE($3, last_name),
                email = COALESCE($4, email),
                role = COALESCE($5, role),
                api_key = COALESCE($6, api_key),
            WHERE user_id = $1
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(user_update.first_name)
        .bind(user_update.last_name)
        .bind(user_update.email)
        .bind(user_update.role)
        .bind(user_update.api_key);

        let user = query.fetch_one(db).await?;
        Ok(user)
    }

    pub async fn delete_user(mm: &ModelManager, user_id: &str) -> Result<u64> {
        let user = sqlx::query(
            r#"
            DELETE FROM users WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .execute(mm.db())
        .await?;

        Ok(user.rows_affected())
    }

    pub async fn get_all_users(mm: &ModelManager) -> Result<Vec<User>> {
        let db = mm.db();
        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT * FROM users
            "#,
        )
        .fetch_all(db)
        .await?;

        Ok(users)
    }
}

// endregion: CRUD

// region: Unit Test

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::ModelManager;
    use crate::error::Result;

    #[tokio::test]
    async fn test_user_bmc() -> Result<()> {
        let mm = ModelManager::new().await?;

        // Create a user
        let new_user = UserForCreate {
            user_id: "test_user".to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            email: "test@email.com".to_string(),
        };
        let created_user = UserBmc::create_user(&mm, new_user.clone()).await?;
        println!("Created User: {:?}", created_user);
        assert_eq!(created_user.user_id, new_user.user_id);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_all_users() -> Result<()> {
        let mm = ModelManager::new().await?;

        // Get all users
        let users = UserBmc::get_all_users(&mm).await?;
        println!("All Users: {:?}", users);
        assert!(!users.is_empty());
        Ok(())
    }
}
// endregion: Unit Test