use crate::error::Result;
use aws_sdk_s3::Client;
use candle_core::Device;
use lib_ai::Embeddings;
use lib_core::database::ModelManager;
use lib_core::model::user::Role;
use lib_cron::ChronJobs;
use lib_storage::create_aws_client;
use moka::future::Cache;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub aws_client: Arc<Client>,
    pub cache_user: Cache<String, UserCacheData>,
    pub cron_jobs: ChronJobs,
    pub embeddings: Arc<Mutex<Embeddings>>,
}

#[derive(Clone, Serialize, Debug)]
pub struct UserCacheData {
    pub user_id: String,
    pub role: Role, // Adjust as needed for tokens usage, requests limits ect.
}

impl AppState {
    pub async fn new(mm: Arc<ModelManager>) -> Result<Self> {
        let client = create_aws_client().await;
        let aws_client = Arc::new(client);
        let cache_user = Cache::builder()
            .time_to_live(std::time::Duration::from_secs(600))
            .build(); //short term cache for user data
        let device = Device::cuda_if_available(0)?;
        let embeddings = Arc::new(Mutex::new(Embeddings::new(
            "danielheinz/e5-base-sts-en-de",
            device,
        )?));
        let cron_jobs = ChronJobs::new(mm, aws_client.clone(), embeddings.clone()).await?;
        Ok(AppState {
            aws_client,
            cache_user,
            cron_jobs,
            embeddings,
        })
    }
}
