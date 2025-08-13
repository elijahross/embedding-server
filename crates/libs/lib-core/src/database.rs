use crate::config::auth_config;
use crate::error::{Error, Result};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tokio::fs::read_to_string;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct ModelManager {
    db: DBPool,
}

pub type DBPool = Pool<Postgres>;

pub async fn init_db_pool() -> Result<DBPool> {
    let config = auth_config();
    new_db_pool(&config.db_url, 5).await
}

pub async fn new_db_pool(db_url: &str, max_con: u32) -> Result<DBPool> {
    PgPoolOptions::new()
        .max_connections(max_con)
        .connect(db_url)
        .await
        .map_err(|ex| Error::FailedToCreatePool(ex.to_string()))
}

pub async fn pexec(pool: &DBPool, file: String) -> Result<()> {
    let query = read_to_string(file.clone())
        .await
        .map_err(|e| println!("Error reading the file, because of {:?}", e))
        .map_err(|_| Error::FileNotFound)?;
    let sql = query.split(';').collect::<Vec<&str>>();
    for q in sql {
        match sqlx::query(q).execute(pool).await {
            Ok(_) => (),
            Err(e) => debug!(
                "Error executing query in file '{}', because of {:?}",
                file, e
            ),
        }
    }
    Ok(())
}

impl ModelManager {
    pub async fn new() -> Result<Self> {
        let db = init_db_pool().await?;
        Ok(Self { db })
    }
    pub fn dev(db: DBPool) -> Self {
        Self { db }
    }

    /// Restrict the pub access to the db field
    pub fn db(&self) -> &DBPool {
        &self.db
    }
}