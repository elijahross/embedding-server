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
