use crate::ai::{Info, infer::Infer};
use crate::error::Result;
use aws_sdk_s3::Client;
use lib_core::database::ModelManager;
use lib_core::model::user::Role;
use lib_cron::ChronJobs;
use lib_storage::create_aws_client;
use moka::future::Cache;
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub aws_client: Arc<Client>,
    pub cache_user: Cache<String, UserCacheData>,
    pub cron_jobs: ChronJobs,
    pub infer: Arc<Infer>,
    pub info: Arc<Info>,
    pub mm: Arc<ModelManager>,
}

#[derive(Clone, Serialize, Debug)]
pub struct UserCacheData {
    pub user_id: String,
    pub role: Role, // Adjust as needed for tokens usage, requests limits ect.
}

impl AppState {
    pub async fn new(mm: Arc<ModelManager>, info: Arc<Info>, infer: Arc<Infer>) -> Result<Self> {
        let client = create_aws_client().await;
        let aws_client = Arc::new(client);
        let cache_user = Cache::builder()
            .time_to_live(std::time::Duration::from_secs(600))
            .build(); //short term cache for user data
        let cron_jobs = ChronJobs::new(mm.clone(), aws_client.clone()).await?;
        Ok(AppState {
            aws_client,
            cache_user,
            cron_jobs,
            infer,
            info,
            mm,
        })
    }
}
