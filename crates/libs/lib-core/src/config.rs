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
    pub db_url: String,
}

impl AuthConfig {
    pub fn load_from_env() -> lib_utils::error::Result<AuthConfig> {
        let db_url = get_env("DATABASE_URL")?;
        Ok(AuthConfig { db_url })
    }
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config() {
        let config = AuthConfig::load_from_env().unwrap();
        assert_eq!(config.db_url.len(), 1);
    }
}

// endregion: Unit Test
