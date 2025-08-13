use crate::database::{new_db_pool, pexec, DBPool};
use crate::error::{Error, Result};
#[allow(unused_imports)]
use tracing::{debug, info};

const DB_CREATE: &str = "postgres://mlapp:mlapp_password@localhost/immotech";
const DB_URL: &str = "postgres://mlapp:mlapp_password@localhost/immotech";

pub async fn init_dev() -> Result<DBPool> {
    info!("Initializing Database _DEV");
    let newpool = new_db_pool(DB_CREATE, 5)
        .await
        .map_err(|e| debug!("Error creating DB_CREATE pool, because of {:?}", e))
        .map_err(|_| Error::DatabaseError)?;
    let _ = pexec(&newpool, "../../../sql/00-init.sql".to_string())
        .await
        .map_err(|e| debug!("Error creating DB user, because of {:?}", e));
    info!("Creating Table and Seeding Data _DEV");
    let pool = new_db_pool(DB_URL, 1)
        .await
        .map_err(|e| debug!("Error creating DB_URL pool, because of {:?}", e))
        .map_err(|_| Error::DatabaseError)?;
    let _ = pexec(&pool, "../../../sql/01-schema.sql".to_string())
        .await
        .map_err(|e| debug!("Error creating DB, because of {:?}", e));
    let _ = pexec(&pool, "../../../sql/02-seed.sql".to_string())
        .await
        .map_err(|e| debug!("Error seeding DB, because of {:?}", e));
    info!("Database Initialization Completed _DEV");

    Ok(pool)
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_dev() {
        let _ = init_dev().await;
    }
}

// endregion: Unit Test