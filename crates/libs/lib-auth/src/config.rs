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
    pub pwd_key: String,
    pub token_duration: u64,
    pub validation_duration: u64,
    pub token_key: String,
}

impl AuthConfig {
    pub fn load_from_env() -> lib_utils::error::Result<AuthConfig> {
        let pwd_key = get_env("AUTH_PWD_KEY")?;
        let token_duration = get_env("TOKEN_DURATION_SEC")?.parse::<u64>()?;
        let validation_duration = get_env("VALIDATION_DURATION_SEC")?.parse::<u64>()?;
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
