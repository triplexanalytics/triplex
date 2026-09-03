//! Partial masking: keeps a fixed prefix and suffix of a value's characters
//! intact and replaces everything between them with `*` — e.g. a card
//! number showing only its last 4 digits. Unicode-aware (counts scalar
//! values, not bytes) so a multi-byte prefix/suffix character isn't split.

use crate::mechanism::Mechanism;

pub(crate) struct PartialMask {
    keep_prefix: usize,
    keep_suffix: usize,
}

impl PartialMask {
    pub(crate) fn new(keep_prefix: usize, keep_suffix: usize) -> Self {
        Self {
            keep_prefix,
            keep_suffix,
        }
    }
}

impl Mechanism for PartialMask {
    fn mask(&self, value: &str) -> String {
        partial_mask(self.keep_prefix, self.keep_suffix, value)
    }
}

/// If `keep_prefix + keep_suffix` already covers the whole value, there's
/// nothing left in the middle to mask, so the value passes through
/// unchanged rather than something misleadingly overlapping.
pub fn partial_mask(keep_prefix: usize, keep_suffix: usize, value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    if keep_prefix + keep_suffix >= len {
        return value.to_string();
    }
    let prefix: String = chars[..keep_prefix].iter().collect();
    let suffix: String = chars[len - keep_suffix..].iter().collect();
    let masked_middle = "*".repeat(len - keep_prefix - keep_suffix);
    format!("{prefix}{masked_middle}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_last_four_digits_of_a_card_number() {
        let card = "4111111111111234";
        let expected = format!("{}1234", "*".repeat(card.len() - 4));
        assert_eq!(partial_mask(0, 4, card), expected);
    }

    #[test]
    fn keeps_both_a_prefix_and_a_suffix() {
        // "abcdefgh" is 8 chars; keeping 2 on each side leaves 4 masked.
        assert_eq!(partial_mask(2, 2, "abcdefgh"), "ab****gh");
    }

    #[test]
    fn a_value_no_longer_than_the_kept_edges_is_unchanged() {
        assert_eq!(partial_mask(2, 2, "ab"), "ab");
        assert_eq!(partial_mask(2, 2, "abc"), "abc");
    }

    #[test]
    fn is_unicode_aware_not_byte_aware() {
        // "café" is 4 chars but 5 bytes (é is 2 bytes) — byte slicing would panic
        // or split the é; char slicing keeps it intact as one unit.
        assert_eq!(partial_mask(1, 1, "café"), "c**é");
    }

    #[test]
    fn zero_prefix_and_suffix_masks_everything() {
        assert_eq!(partial_mask(0, 0, "secret"), "******");
    }
}
