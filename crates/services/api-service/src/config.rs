use lib_utils::envs::get_env;
use std::sync::OnceLock;

pub fn auth_config() -> &'static AuthConfig {
    static INSTANCE: OnceLock<AuthConfig> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        AuthConfig::load_from_env()
            .unwrap_or_else(|e| panic!("Failed while loading AuthConfig from env - Cause: {e:?}"))
    })
}

pub struct AuthConfig {
    pub bucket: String,
    pub hash_salt: uuid::Uuid,
    pub api_key: String,
}

impl AuthConfig {
    pub fn load_from_env() -> lib_utils::error::Result<AuthConfig> {
        let bucket = get_env("UPLOAD_BUCKET")?;
        let hash_salt = get_env("HASH_SALT")?;
        let api_key = get_env("API_KEY")?;
        Ok(AuthConfig {
            bucket,
            hash_salt,
            api_key,
        })
    }
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config() {
        let config = AuthConfig::load_from_env().unwrap();
        assert_eq!(config.bucket.len(), 1);
    }
}

// endregion: Unit Test
