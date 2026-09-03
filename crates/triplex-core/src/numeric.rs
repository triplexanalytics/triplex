use std::borrow::Cow;

/// Accounting/mainframe-style exports put the sign after the number
/// (`"150.00-"`, `"42+"`) instead of before it. `str::parse` for `i64`/`f64`
/// only accepts a leading sign, so every numeric parse across the ingestion
/// formats runs its input through this first. A value with no trailing sign
/// (or nothing but a sign) comes back unchanged, so it fails to parse exactly
/// as it did before this existed.
pub fn normalize_trailing_sign(v: &str) -> Cow<'_, str> {
    if let Some(rest) = v.strip_suffix('-') {
        if !rest.is_empty() {
            return Cow::Owned(format!("-{rest}"));
        }
    } else if let Some(rest) = v.strip_suffix('+') {
        if !rest.is_empty() {
            return Cow::Borrowed(rest);
        }
    }
    Cow::Borrowed(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_a_trailing_minus_to_the_front() {
        assert_eq!(normalize_trailing_sign("150.00-"), "-150.00");
        assert_eq!(normalize_trailing_sign("42-"), "-42");
    }

    #[test]
    fn drops_a_trailing_plus() {
        assert_eq!(normalize_trailing_sign("42+"), "42");
    }

    #[test]
    fn leaves_unsigned_and_leading_signed_values_alone() {
        assert_eq!(normalize_trailing_sign("42"), "42");
        assert_eq!(normalize_trailing_sign("-42"), "-42");
        assert_eq!(normalize_trailing_sign("+42"), "+42");
    }

    #[test]
    fn leaves_a_lone_sign_or_non_numeric_text_alone() {
        assert_eq!(normalize_trailing_sign("-"), "-");
        assert_eq!(normalize_trailing_sign("+"), "+");
        assert_eq!(normalize_trailing_sign("n/a-"), "-n/a");
    }
}
