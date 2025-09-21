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
    pub parser: String,
    pub bucket: String,
    pub max_tokens: i16,
}

impl AuthConfig {
    pub fn load_from_env() -> lib_utils::error::Result<AuthConfig> {
        let parser = get_env("PARSER_URL")?;
        let bucket = get_env("UPLOAD_BUCKET")?;
        let max_tokens = get_env("MAX_TOKENS")?;
        Ok(AuthConfig {
            parser,
            bucket,
            max_tokens,
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
