use aws_sdk_s3::Client;
use chrono::Utc;
use lib_core::model::{
    settings::Settings,
    users::{Role, User},
};
use lib_ai::Embeddings;
use lib_storage::create_aws_client;
use moka::future::Cache;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use lib_cron::ChronJobs;


#[derive(Clone)]
pub struct AppState {
    pub aws_client: Arc<Mutex<Client>>,
    pub cache_user: Cache<String, UserCacheData>,
    pub cron_jobs: ChronJobs,
    pub embeddings: Arc<Embeddings>,
}

#[derive(Clone, Serialize, Debug)]
pub struct UserCacheData {
    pub user_id: String,
    pub role: Role,
    pub user: User,
    pub settings: Settings,
}


impl AppState {
    pub async fn _new() -> Self {
        let client = create_aws_client().await;
        let aws_client = Arc::new(Mutex::new(client));
        let cache_user = Cache::builder()
            .time_to_live(std::time::Duration::from_secs(60))
            .build();
        let cron_jobs = ChronJobs::new().await?;
        let embeddings = Arc::new(Embeddings::new("danielheinz/e5-base-sts-en-de").await?);
        AppState {
            aws_client,
            cache_user,
            cron_jobs,
            embeddings
        }
    }
}