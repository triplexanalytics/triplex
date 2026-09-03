//! Format-preserving "encryption": a keyed, per-position substitution over
//! three character classes (digits, lowercase, uppercase) that keeps a
//! value's length and character shape intact — e.g. a 16-digit card number
//! stays a 16-digit number, so downstream systems/validators expecting that
//! shape keep working. Deterministic and salt-dependent like
//! `tokenize::hmac_token`, but format-preserving where HMAC's fixed-length
//! hex output isn't.
//!
//! This is a keyed substitution cipher, not a block-cipher-based scheme
//! like NIST FF1/FF3-1: for a fixed position it's a straight permutation of
//! that position's alphabet, which is weaker against frequency analysis on
//! a column with many repeated values than a true block cipher (folding the
//! position into the keyed permutation at least stops the same character
//! from mapping to the same output at every position). That's an
//! acceptable trade for Tier 1's stated purpose — format compatibility, not
//! a standalone cryptographic guarantee — since Tier 1's reversibility
//! already runs through the mapping file (see `mapping.rs`), not this
//! cipher's own invertibility.
//! ponytail: no `unmask`/decrypt entry point — nothing in this crate needs
//! one, since the mapping file is the reversal path for every Tier 1
//! mechanism. Add one if a caller ever needs to invert this independent of
//! the mapping file.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::mechanism::Mechanism;

type HmacSha256 = Hmac<Sha256>;

const DIGITS: &[u8] = b"0123456789";
const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";

pub(crate) struct Fpe {
    key: Vec<u8>,
}

impl Fpe {
    pub(crate) fn new(key: &[u8]) -> Self {
        Self { key: key.to_vec() }
    }
}

impl Mechanism for Fpe {
    fn mask(&self, value: &str) -> String {
        fpe_mask(&self.key, value)
    }
}

/// Substitutes every ASCII digit/letter in `value` for another character of
/// the same class, keyed by `key` and each character's position. Anything
/// outside those three classes (punctuation, separators, non-ASCII) passes
/// through unchanged, which is what keeps a formatted value's shape (e.g.
/// `"020-123-4567"`'s dashes) intact.
pub fn fpe_mask(key: &[u8], value: &str) -> String {
    value
        .chars()
        .enumerate()
        .map(|(position, c)| substitute(key, position, c))
        .collect()
}

fn alphabet_for(c: char) -> Option<&'static [u8]> {
    if c.is_ascii_digit() {
        Some(DIGITS)
    } else if c.is_ascii_lowercase() {
        Some(LOWER)
    } else if c.is_ascii_uppercase() {
        Some(UPPER)
    } else {
        None
    }
}

/// A keyed Fisher-Yates permutation of `alphabet`, seeded by HMAC-SHA256
/// over `key` and `position` — every position gets its own permutation.
fn keyed_permutation(key: &[u8], position: usize, alphabet: &[u8]) -> Vec<u8> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(&position.to_le_bytes());
    let seed = mac.finalize().into_bytes();

    let mut permuted = alphabet.to_vec();
    for i in (1..permuted.len()).rev() {
        // Cycles the 32-byte HMAC output as a stream of swap indices — the
        // alphabet never exceeds 26 characters, so one byte of range is
        // always enough for `% (i + 1)`.
        let byte = seed[i % seed.len()];
        let j = (byte as usize) % (i + 1);
        permuted.swap(i, j);
    }
    permuted
}

fn substitute(key: &[u8], position: usize, c: char) -> char {
    let Some(alphabet) = alphabet_for(c) else {
        return c; // not a digit/letter — pass through unchanged
    };
    let permuted = keyed_permutation(key, position, alphabet);
    let index = alphabet
        .iter()
        .position(|&b| b == c as u8)
        .expect("c came from alphabet_for(c), so it is in its own alphabet");
    permuted[index] as char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_deterministic_for_the_same_key_and_value() {
        assert_eq!(fpe_mask(b"key", "4111 1111"), fpe_mask(b"key", "4111 1111"));
    }

    #[test]
    fn preserves_length_and_character_class_per_position() {
        let masked = fpe_mask(b"key", "AB-12-cd");
        assert_eq!(masked.len(), "AB-12-cd".len());
        for (original, out) in "AB-12-cd".chars().zip(masked.chars()) {
            match original {
                c if c.is_ascii_digit() => assert!(out.is_ascii_digit()),
                c if c.is_ascii_lowercase() => assert!(out.is_ascii_lowercase()),
                c if c.is_ascii_uppercase() => assert!(out.is_ascii_uppercase()),
                _ => assert_eq!(out, original), // separators pass through as-is
            }
        }
    }

    #[test]
    fn differs_across_keys_for_the_same_value() {
        assert_ne!(
            fpe_mask(b"key-one", "4111111111111111"),
            fpe_mask(b"key-two", "4111111111111111")
        );
    }

    #[test]
    fn masks_at_least_one_character_of_a_realistic_value() {
        // A single character could coincidentally map to itself, but a
        // 16-digit value mapping to exactly itself would be a broken cipher.
        assert_ne!(fpe_mask(b"key", "4111111111111111"), "4111111111111111");
    }

    #[test]
    fn the_same_character_at_different_positions_can_map_differently() {
        let masked = fpe_mask(b"key", "0000");
        let chars: Vec<char> = masked.chars().collect();
        assert!(
            chars.iter().any(|&c| c != chars[0]),
            "expected position-dependent substitution to break up a run of identical input \
             characters, got {masked}"
        );
    }
}
