//! Numeric generalization/bucketing — the k-anonymity mechanism from
//! the design notes's Tier 1 row. The named risk models a
//! bucketed column can be checked against (`RiskModel`/`evaluate_risk_model`
//! and friends) moved to `triplex-datasetcard` — see the design notes —
//! since scoring risk needs to run independently of any recipe, before a
//! masking mechanism is even chosen.

use crate::mechanism::Mechanism;

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

/// Replaces a numeric value with the `"lo-hi"` range it falls in, e.g.
/// width 10 turns `"23"` into `"20-29"`. A value that isn't a whole number
/// has nothing to generalize and passes through unchanged.
fn bucket_number(width: u64, n: i64) -> String {
    let width = width.max(1) as i64;
    let lo = n.div_euclid(width) * width;
    let hi = lo + width - 1;
    format!("{lo}-{hi}")
}

pub fn bucket_value(width: u64, value: &str) -> String {
    let Ok(n) = value.parse::<i64>() else {
        return value.to_string();
    };
    bucket_number(width, n)
}

/// Same bucketing as `bucket_value`, for a caller that already has a typed
/// numeric value instead of a string to parse (e.g. `apps/triplex-web`'s
/// risk-scoring wasm exports, which take a typed-array of `f64` straight
/// from the browser instead of a JSON string of stringified cells — see
/// the design notes follow-up). `NaN` represents a
/// missing/non-numeric cell, passed through as an empty string, the same
/// convention `bucket_value` uses for a value that fails to parse.
pub fn bucket_numeric(width: u64, value: f64) -> String {
    if value.is_nan() {
        return String::new();
    }
    bucket_number(width, value as i64)
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
/// still deliberately unbuilt, see the design notes.
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
    fn buckets_a_number_into_its_width_sized_range() {
        assert_eq!(bucket_value(10, "23"), "20-29");
        assert_eq!(bucket_value(10, "20"), "20-29");
        assert_eq!(bucket_value(10, "29"), "20-29");
        assert_eq!(bucket_value(10, "30"), "30-39");
    }

    #[test]
    fn negative_numbers_bucket_toward_negative_infinity_not_toward_zero() {
        // div_euclid, not integer truncation: -1 must land in "-10--1", not "0-9".
        assert_eq!(bucket_value(10, "-1"), "-10--1");
    }

    #[test]
    fn a_non_numeric_value_passes_through_unchanged() {
        assert_eq!(bucket_value(10, "n/a"), "n/a");
    }

    #[test]
    fn bucket_numeric_matches_bucket_value_for_the_same_number() {
        assert_eq!(bucket_numeric(10, 23.0), bucket_value(10, "23"));
        assert_eq!(bucket_numeric(10, -1.0), bucket_value(10, "-1"));
    }

    #[test]
    fn bucket_numeric_treats_nan_as_the_missing_value_case() {
        assert_eq!(bucket_numeric(10, f64::NAN), "");
    }

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
