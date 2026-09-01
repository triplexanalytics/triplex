use aes_gcm::aead::{rand_core::RngCore, Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use triplex_core::{Error, Result};

/// One reversible entry in the mapping file: which column, the surrogate
/// token that now sits in the anonymized output, and the real value it
/// stands for. One entry per distinct (column, original_value) pair that
/// actually appeared in the data — not one per row.
#[derive(Debug, Serialize)]
pub struct MappingEntry {
    pub column: String,
    pub token: String,
    pub original_value: String,
}

/// Serializes mapping entries as CSV bytes (`column,token,original_value`).
pub fn mapping_to_csv(entries: &[MappingEntry]) -> Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    for entry in entries {
        writer
            .serialize(entry)
            .map_err(|e| Error::Csv(e.to_string()))?;
    }
    writer.into_inner().map_err(|e| Error::Csv(e.to_string()))
}

/// A mapping file encrypted with a client-supplied passphrase. Self-describing
/// (names its own KDF) and holds no secret — the passphrase is never stored,
/// only known to whoever runs `anonymize` with it and needs it again later.
#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptedMapping {
    pub kdf: String,
    pub kdf_salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

const KDF_SALT_LEN: usize = 16;
const AES_KEY_LEN: usize = 32;

fn derive_key(passphrase: &str, kdf_salt: &[u8]) -> Result<[u8; AES_KEY_LEN]> {
    let mut key = [0u8; AES_KEY_LEN];
    argon2::Argon2::default()
        .hash_password_into(passphrase.as_bytes(), kdf_salt, &mut key)
        .map_err(|e| Error::Crypto(format!("key derivation failed: {e}")))?;
    Ok(key)
}

/// Encrypts mapping bytes with AES-256-GCM, deriving the key from
/// `passphrase` via Argon2id with a freshly generated salt.
pub fn encrypt_mapping(plaintext: &[u8], passphrase: &str) -> Result<EncryptedMapping> {
    let mut kdf_salt = [0u8; KDF_SALT_LEN];
    OsRng.fill_bytes(&mut kdf_salt);

    let key_bytes = derive_key(passphrase, &kdf_salt)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| Error::Crypto(format!("encryption failed: {e}")))?;

    Ok(EncryptedMapping {
        kdf: "argon2id".to_string(),
        kdf_salt: BASE64.encode(kdf_salt),
        nonce: BASE64.encode(nonce),
        ciphertext: BASE64.encode(ciphertext),
    })
}

/// Decrypts a mapping previously produced by `encrypt_mapping`. A wrong
/// passphrase fails as a generic AEAD authentication error — that's
/// inherent to AES-GCM, not a gap in this code.
pub fn decrypt_mapping(enc: &EncryptedMapping, passphrase: &str) -> Result<Vec<u8>> {
    let decode = |s: &str, what: &str| {
        BASE64
            .decode(s)
            .map_err(|e| Error::Crypto(format!("invalid {what}: {e}")))
    };
    let kdf_salt = decode(&enc.kdf_salt, "kdf_salt")?;
    let nonce_bytes = decode(&enc.nonce, "nonce")?;
    let ciphertext = decode(&enc.ciphertext, "ciphertext")?;

    let key_bytes = derive_key(passphrase, &kdf_salt)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| Error::Crypto("decryption failed (wrong passphrase or corrupted file)".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_csv_round_trips_its_fields() {
        let entries = vec![MappingEntry {
            column: "email".into(),
            token: "abc123".into(),
            original_value: "alice@example.com".into(),
        }];
        let bytes = mapping_to_csv(&entries).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("email,abc123,alice@example.com"));
    }

    #[test]
    fn encrypt_then_decrypt_recovers_the_original_bytes() {
        let plaintext = b"column,token,original_value\nemail,abc123,alice@example.com\n";
        let enc = encrypt_mapping(plaintext, "correct horse battery staple").unwrap();
        let decrypted = decrypt_mapping(&enc, "correct horse battery staple").unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_passphrase_fails_to_decrypt() {
        let plaintext = b"secret mapping data";
        let enc = encrypt_mapping(plaintext, "right passphrase").unwrap();
        let result = decrypt_mapping(&enc, "wrong passphrase");
        assert!(result.is_err());
    }

    #[test]
    fn two_encryptions_of_the_same_plaintext_use_different_salts_and_nonces() {
        let plaintext = b"same data every time";
        let a = encrypt_mapping(plaintext, "pw").unwrap();
        let b = encrypt_mapping(plaintext, "pw").unwrap();
        assert_ne!(a.kdf_salt, b.kdf_salt);
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
    }
}
