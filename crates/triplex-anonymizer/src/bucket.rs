//! Numeric generalization/bucketing — the Tier 1 k-anonymity
//! mechanism. The bucketing itself (`bucket_value`, `bucket_numeric`) lives in
//! `triplex-datasetcard`, so masking and risk scoring generalize a number the
//! same way; this module wraps it as a masking mechanism. The named risk models a
//! bucketed column can be checked against (`RiskModel`/`evaluate_risk_model`
//! and friends) live in `triplex-datasetcard`,
//! since scoring risk needs to run independently of any recipe, before a
//! masking mechanism is even chosen.

use crate::mechanism::Mechanism;
use triplex_datasetcard::bucket_value;

pub(crate) struct Bucketer {
    width: u64,
}

impl Bucketer {
    pub(crate) fn new(width: u64) -> Self {
        Self {
            width: width.max(1),
        }
    }
}

impl Mechanism for Bucketer {
    fn mask(&self, value: &str) -> String {
        bucket_value(self.width, value)
    }
}

/// Generalizes a string value by keeping only its first `width` characters
/// (Unicode scalar values, not bytes, so a multi-byte character is never
/// split mid-codepoint) — the "automatic" string generalization ARX ships
/// (left-to-right substring), useful for structurally-hierarchical strings
/// (postal codes, phone numbers, IDs, IBANs) where a shared prefix is a
/// real coarser category. Unlike `bucket_value`'s numeric ranges, this is
/// *not* a good default for free-text categorical values (city names,
/// plan tiers): truncating "Utrecht"/"Rotterdam" to 3 characters doesn't
/// merge them into a meaningful shared category the way a hand-authored
/// taxonomy (city → province → country) would — that taxonomy mechanism is
/// still deliberately unbuilt.
/// `width == 0` means "no truncation," matching a bucket width of 0 having
/// no meaningful bucket: the value passes through unchanged, the same
/// behavior `score_column_risk_string`'s exact-value grouping already had
/// before this function existed.
pub fn generalize_string_prefix(width: usize, value: &str) -> String {
    if width == 0 {
        return value.to_string();
    }
    value.chars().take(width).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generalize_string_prefix_keeps_only_the_leading_characters() {
        assert_eq!(generalize_string_prefix(4, "3512AD"), "3512");
        assert_eq!(generalize_string_prefix(4, "AB"), "AB"); // shorter than width: unchanged
    }

    #[test]
    fn generalize_string_prefix_zero_width_means_no_truncation() {
        assert_eq!(generalize_string_prefix(0, "Utrecht"), "Utrecht");
    }

    #[test]
    fn generalize_string_prefix_counts_unicode_characters_not_bytes() {
        // "café" is 4 chars but 5 bytes (é is 2 bytes in UTF-8) — a byte-based
        // truncation would either panic (mid-codepoint) or silently corrupt
        // the last character. Keeping the first 3 characters must yield "caf".
        assert_eq!(generalize_string_prefix(3, "café"), "caf");
    }
}
