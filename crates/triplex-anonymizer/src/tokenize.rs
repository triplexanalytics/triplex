use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::mechanism::Mechanism;

type HmacSha256 = Hmac<Sha256>;

pub(crate) struct HmacTokenizer {
    salt: Vec<u8>,
}

impl HmacTokenizer {
    pub(crate) fn new(salt: &[u8]) -> Self {
        Self {
            salt: salt.to_vec(),
        }
    }
}

impl Mechanism for HmacTokenizer {
    fn mask(&self, value: &str) -> String {
        hmac_token(&self.salt, value)
    }
}

/// Deterministic HMAC-SHA256 tokenization: the same value under the same
/// salt always produces the same token, so repeated entity references stay
/// consistent across a run without needing a lookup. Hex-encoded (64 chars)
/// rather than base64 so tokens are safe to embed directly in CSV/Parquet
/// text columns with no escaping concerns.
pub fn hmac_token(salt: &[u8], value: &str) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(salt).expect("HMAC accepts any key length");
    mac.update(value.as_bytes());
    hex_encode(&mac.finalize().into_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_deterministic_for_the_same_salt_and_value() {
        assert_eq!(
            hmac_token(b"salt", "alice@example.com"),
            hmac_token(b"salt", "alice@example.com")
        );
    }

    #[test]
    fn differs_across_values_under_the_same_salt() {
        assert_ne!(
            hmac_token(b"salt", "alice@example.com"),
            hmac_token(b"salt", "bob@example.com")
        );
    }

    #[test]
    fn differs_across_salts_for_the_same_value() {
        assert_ne!(
            hmac_token(b"salt-one", "alice@example.com"),
            hmac_token(b"salt-two", "alice@example.com")
        );
    }

    #[test]
    fn is_a_64_character_lowercase_hex_string() {
        let token = hmac_token(b"salt", "alice@example.com");
        assert_eq!(token.len(), 64);
        assert!(token
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
