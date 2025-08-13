pub mod config;
pub mod error;
pub mod functions;

use crate::config::config;
use aws_config::meta::region::RegionProviderChain;
use aws_credential_types::provider::ProvideCredentials;
use aws_sdk_s3::{
    config::{BehaviorVersion, Credentials, Region},
    Client,
};

#[derive(Debug)]
struct StaticCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl StaticCredentials {
    pub fn new() -> Self {
        Self {
            access_key_id: config().aws_access_key_id.to_string(),
            secret_access_key: config().aws_access_key.to_string(),
        }
    }

    async fn load_credentials(&self) -> aws_credential_types::provider::Result {
        Ok(Credentials::new(
            self.access_key_id.clone(),
            self.secret_access_key.clone(),
            None,
            None,
            "StaticCredentials",
        ))
    }
}
impl ProvideCredentials for StaticCredentials {
    fn provide_credentials<'a>(
        &'a self,
    ) -> aws_credential_types::provider::future::ProvideCredentials<'a>
    where
        Self: 'a,
    {
        aws_credential_types::provider::future::ProvideCredentials::new(self.load_credentials())
    }
}

pub async fn create_aws_client() -> Client {
    let region_provider =
        RegionProviderChain::first_try(Region::new(config().aws_region.to_string()));
    let cred = StaticCredentials::new();
    let shared_config = aws_config::defaults(BehaviorVersion::v2025_01_17())
        .region(region_provider)
        .credentials_provider(cred)
        .load()
        .await;

    Client::new(&shared_config)
}