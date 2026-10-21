//! Numeric generalization: a number replaced by the `"lo-hi"` range it falls in. It lives here,
//! next to the risk models, because scoring a column's risk generalizes it the same way the
//! Anonymizer's bucketing masks it, and the two have to agree.

/// Replaces a whole number with the `"lo-hi"` range it falls in, e.g. width 10 turns 23 into
/// `"20-29"`. Ranges run toward negative infinity, so -1 lands in `"-10--1"`, not `"0-9"`.
fn bucket_number(width: u64, n: i64) -> String {
    let width = width.max(1) as i64;
    let lo = n.div_euclid(width) * width;
    let hi = lo + width - 1;
    format!("{lo}-{hi}")
}

/// Buckets a value given as text. A value that isn't a whole number has nothing to generalize
/// and passes through unchanged.
pub fn bucket_value(width: u64, value: &str) -> String {
    let Ok(n) = value.parse::<i64>() else {
        return value.to_string();
    };
    bucket_number(width, n)
}

/// Same bucketing as `bucket_value`, for a caller that already has a typed number (for example
/// the web UI's risk scoring, which takes a typed array of `f64`). `NaN` marks a missing or
/// non-numeric cell and becomes an empty string, the same convention `bucket_value` uses for a
/// value that fails to parse.
pub fn bucket_numeric(width: u64, value: f64) -> String {
    if value.is_nan() {
        return String::new();
    }
    bucket_number(width, value as i64)
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
}
