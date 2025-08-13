use crate::config;
use crate::error::{Error, Result};
use hmac::{Hmac, Mac};
use lib_utils::base64::b64u_encode;
use sha2::Sha256;
use uuid::Uuid;

pub struct ContentToHash {
    pub content: String,
    pub salt: Uuid,
}

pub fn hash_key(content: ContentToHash) -> Result<String> {
    let key = &config::auth_config().pwd_key;
    let mut mac = Hmac::<Sha256>::new_from_slice(key).map_err(|_| Error::HmacFailNewFromSlice)?;
    mac.update(content.content.as_bytes());
    mac.update(content.salt.as_bytes());
    let result = mac.finalize().into_bytes();
    let res = b64u_encode(result);
    Ok(format!("#01#{}", res))
}

pub fn validate_key(content: ContentToHash, pwd_check: String) -> Result<()> {
    let hash = hash_key(content)?;
    if hash != pwd_check {
        return Err(Error::InvalidPassword);
    }
    Ok(())
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_key() -> Result<()> {
        let salt = Uuid::new_v4();
        let content = ContentToHash {
            content: "password".to_string(),
            salt: salt.clone(),
        };
        let content_check = ContentToHash {
            content: "wrong_password".to_string(),
            salt: salt,
        };
        let res = hash_key(content).unwrap();
        let negative_test = validate_key(content_check, res);
        assert!(negative_test.is_err());
        Ok(())
    }
}
// endregion: Unit Test
