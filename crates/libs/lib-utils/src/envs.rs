use crate::base64::b64u_decode;
use crate::error::{Error, Result};
use std::env;
use std::str::FromStr;

pub fn get_env<T: FromStr>(key: &'static str) -> Result<T> {
    env::var(key)
        .map_err(|_| Error::MissingEnv(key))
        .and_then(|v| v.parse().map_err(|_| Error::WrongFormat(key)))
}

pub fn get_env_b64u_as_u8s(key: &'static str) -> Result<Vec<u8>> {
    env::var(key)
        .map_err(|_| Error::MissingEnv(key))
        .and_then(|v| b64u_decode(&v))
        .map_err(|_| Error::WrongFormat(key))
}
