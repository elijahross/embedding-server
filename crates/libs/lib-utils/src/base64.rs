use crate::error::{Error, Result};
use base64::engine::{general_purpose, Engine};

pub fn b64u_encode(data: impl AsRef<[u8]>) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(data)
}

pub fn b64u_decode(data: &str) -> Result<Vec<u8>> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(data)
        .map_err(|_| Error::FailToB64uDecode)
}

pub fn b64u_decode_to_string(data: &str) -> Result<String> {
    b64u_decode(data)
        .ok()
        .and_then(|v| String::from_utf8(v).ok())
        .ok_or(Error::FailToB64uDecode)
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b64u_encode() {
        let data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let encoded = b64u_encode(&data);
        assert_eq!(encoded, "AAECAwQFBgcICQ");
    }

    #[test]
    fn test_b64u_decode() {
        let data = "AAECAwQFBgcICQ";
        let decoded = b64u_decode(data).unwrap();
        assert_eq!(decoded, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_b64u_decode_to_string() {
        let data = "AAECAwQFBgcICQ";
        let decoded = b64u_decode_to_string(data);
        assert!(decoded.is_ok());
    }
}

// endregion: Unit Test
