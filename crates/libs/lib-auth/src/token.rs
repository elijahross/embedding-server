use crate::config::auth_config;
use crate::error::{Error, Result};
use hmac::{Hmac, Mac};
use lib_utils::base64::{b64u_decode_to_string, b64u_encode};
use lib_utils::time::{now_utc, now_utc_plus_sec, parse_time};
use sha2::Sha256;
use std::fmt::Display;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug)]
pub struct Token {
    pub ident: String,
    pub exp: String,
    pub sign: String,
}

impl FromStr for Token {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(Error::InvalidTokenFormat);
        }

        let ident = b64u_decode_to_string(parts[0]).map_err(|_| Error::CannotDecodeIdent)?;
        let exp = b64u_decode_to_string(parts[1]).map_err(|_| Error::CannotDecodeExp)?;
        let sign = parts[2].to_string();

        Ok(Self { ident, exp, sign })
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}",
            b64u_encode(&self.ident),
            b64u_encode(&self.exp),
            self.sign
        )
    }
}

pub fn generate_web_token(user_id: &str, salt: &Uuid) -> Result<Token> {
    let config = auth_config();
    _generate_token(user_id, config.token_duration, salt, &config.token_key)
}

pub fn generate_validation_token(user_id: &str, salt: &str) -> Result<Token> {
    let config = auth_config();
    _generate_validation_token(user_id, config.validation_duration, salt, &config.token_key)
}

pub fn validate_web_token(token: &Token, salt: &Uuid) -> Result<()> {
    let config = auth_config();
    _validate_token(token, salt, &config.token_key)
}

pub fn validate_validation_token(token: &Token, salt: &str) -> Result<()> {
    let config = auth_config();
    _validate_validation_token(token, salt, &config.token_key)
}

fn _generate_token(ident: &str, duration: i64, salt: &Uuid, key: &[u8]) -> Result<Token> {
    let ident = ident.to_string();
    let exp = now_utc_plus_sec(duration);
    let sign = _sign_token(&ident, &exp, salt, key)?;
    Ok(Token { ident, exp, sign })
}

fn _generate_validation_token(
    user_id: &str,
    duration: i64,
    salt: &str,
    key: &[u8],
) -> Result<Token> {
    let ident = user_id.to_string();
    let exp = now_utc_plus_sec(duration);
    let sign = _sign_validation_token(&ident, &exp, salt, key)?;
    Ok(Token { ident, exp, sign })
}

fn _validate_token(token: &Token, salt: &Uuid, key: &[u8]) -> Result<()> {
    let sign = _sign_token(&token.ident, &token.exp, salt, key)?;
    if sign != token.sign {
        return Err(Error::SignatureNotMatching);
    }
    let exp_check = parse_time(&token.exp).map_err(|_| Error::ExpNotIso)?;
    if exp_check < now_utc() {
        return Err(Error::Expired);
    }
    Ok(())
}

fn _validate_validation_token(token: &Token, salt: &str, key: &[u8]) -> Result<()> {
    let sign = _sign_validation_token(&token.ident, &token.exp, salt, key)?;
    if sign != token.sign {
        return Err(Error::SignatureNotMatching);
    }
    let exp_check = parse_time(&token.exp).map_err(|_| Error::ExpNotIso)?;
    if exp_check < now_utc() {
        return Err(Error::Expired);
    }
    Ok(())
}

fn _sign_token(ident: &str, exp: &str, salt: &Uuid, key: &[u8]) -> Result<String> {
    let content = format!("{}.{}", b64u_encode(ident), b64u_encode(exp));
    let mut mac = Hmac::<Sha256>::new_from_slice(key).map_err(|_| Error::HmacFailNewFromSlice)?;
    mac.update(content.as_bytes());
    mac.update(salt.as_bytes());
    let result = mac.finalize().into_bytes();
    Ok(b64u_encode(result))
}

fn _sign_validation_token(ident: &str, exp: &str, salt: &str, key: &[u8]) -> Result<String> {
    let content = format!("{}.{}", b64u_encode(ident), b64u_encode(exp));
    let mut mac = Hmac::<Sha256>::new_from_slice(key).map_err(|_| Error::HmacFailNewFromSlice)?;
    mac.update(content.as_bytes());
    mac.update(salt.as_bytes());
    let result = mac.finalize().into_bytes();
    Ok(b64u_encode(result))
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_token() -> Result<()> {
        let salt = Uuid::new_v4();
        let token = generate_web_token("user_id", &salt).unwrap();
        println!("Token: {}", token.to_string());
        validate_web_token(&token, &salt)
    }

    #[test]
    fn test_validation_token() -> Result<()> {
        let salt = "123456";
        let salt2 = "123456";
        let user_id = "75";
        let token = generate_validation_token(user_id, salt).unwrap();
        println!("Token: {}", token.to_string());
        validate_validation_token(&token, salt2)
    }
}

// endregion: Unit Test
