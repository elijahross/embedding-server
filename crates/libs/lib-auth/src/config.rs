use lib_utils::envs::get_env;
use std::sync::OnceLock;
use tracing::error;

pub fn config() -> &'static Config {
    static INSTANCE: OnceLock<Config> = OnceLock::new();
    INSTANCE.get_or_init(|| match Config::load_from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed while loading configuration - Cause: {e:?}");
            Config::default()
        }
    })
}

pub struct AuthConfig {
    pub pwd_key: String,
    pub token_duration: u64,
    pub validation_duration: u64,
    pub token_key: String,
}

impl AuthConfig {
    pub fn load_from_env() -> lib_utils::error::Result<AuthConfig> {
        let pwd_key = get_env("AUTH_PWD_KEY")?;
        let token_duration = get_env::<u64>("TOKEN_DURATION_SEC")?;
        let validation_duration = get_env::<u64>("VALIDATION_DURATION_SEC")?;
        let token_key = get_env("AUTH_TOKEN_KEY")?;
        Ok(AuthConfig {
            pwd_key,
            token_duration,
            validation_duration,
            token_key,
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
        assert_eq!(config.pwd_key.len(), 1);
    }
}

// endregion: Unit Test
