use lib_utils::envs::get_env;
use std::sync::OnceLock;
use tracing::error;

pub fn config() -> &'static Config {
    static INSTANCE: OnceLock<Config> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        match Config::load_from_env() {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("Failed while loading configuration - Cause: {e:?}");
                Config::default() // <- fallback (requires implementing Default)
            }
        }
    })
}

pub struct Config {
    pub aws_region: String,
    pub aws_access_key: String,
    pub aws_access_key_id: String,
}

impl Config {
    fn load_from_env() -> lib_utils::error::Result<Config> {
        Ok(Config {
            aws_region: get_env("AM_REGION")?,
            aws_access_key: get_env("AM_ACCESS_KEY")?,
            aws_access_key_id: get_env("AM_ACCESS_KEY_ID")?,
        })
    }
}
