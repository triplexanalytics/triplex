use std::borrow::Cow;

const CURRENCY_SYMBOLS: [char; 5] = ['$', '€', '£', '¥', '¢'];

fn is_special(c: char) -> bool {
    matches!(c, '-' | '+' | '(' | ')' | ',' | '.') || CURRENCY_SYMBOLS.contains(&c)
}

/// Moves a trailing sign to the front: accounting/mainframe-style exports
/// write `"150.00-"`/`"42+"` instead of `"-150.00"`/`"42"`, but `str::parse`
/// for `i64`/`f64` only accepts a leading sign.
fn normalize_trailing_sign(v: &str) -> Cow<'_, str> {
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

/// Resolves thousands separators and a comma decimal point in a string that
/// already has any currency symbol, parentheses, and trailing sign removed.
///
/// Two or more occurrences of the same separator are unambiguously thousands
/// grouping (`"1,234,567"`). One `,` and one `.` both present: whichever
/// comes last is the decimal point (`"1.234,56"` and `"1,234.56"` both read
/// as `1234.56`). Exactly one separator, on its own: a single comma is a
/// decimal point unless it groups exactly 3 trailing digits, in which case
/// it reads as thousands grouping rather than a 3-decimal amount — genuinely
/// ambiguous (`"1,234"` could be `1234` or a 3-decimal-currency `1.234`),
/// resolved toward the far more common case. A single `.` is already a
/// valid decimal point unless it has that same 3-digit shape.
fn normalize_separators(s: &str) -> String {
    let comma_count = s.matches(',').count();
    let dot_count = s.matches('.').count();

    if comma_count > 0 && dot_count > 0 {
        let decimal_is_comma = s.rfind(',').unwrap() > s.rfind('.').unwrap();
        let thousands = if decimal_is_comma { '.' } else { ',' };
        let cleaned: String = s.chars().filter(|&c| c != thousands).collect();
        return if decimal_is_comma {
            cleaned.replace(',', ".")
        } else {
            cleaned
        };
    }
    if comma_count > 1 {
        return s.replace(',', "");
    }
    if dot_count > 1 {
        return s.replace('.', "");
    }

    let sep = if comma_count == 1 { ',' } else { '.' };
    let idx = match s.rfind(sep) {
        Some(idx) => idx,
        None => return s.to_string(),
    };
    let trailing_digits = s.len() - idx - 1;
    if trailing_digits == 3 {
        return s.chars().filter(|&c| c != sep).collect();
    }
    if sep == ',' {
        return s.replace(',', ".");
    }
    s.to_string()
}

/// Normalizes numeric-string conventions that `str::parse` for `i64`/`f64`
/// rejects outright, so every numeric parse across the ingestion formats
/// runs its input through this first: a trailing sign (`"150.00-"`), a
/// parenthesized negative (`"(150.00)"`), a leading or trailing currency
/// symbol (`"$150"`), and thousands separators or a comma decimal point
/// (`"1,234.56"`, `"1.234,56"`, `"150,00"`). A value using none of these
/// conventions comes back unchanged, so it fails to parse exactly as it did
/// before this existed.
///
/// ponytail: currency symbol *and* parentheses combined (`"($150.00)"`) is
/// not resolved — each convention is handled on its own, not every nested
/// combination. Add that case if a real export needs it.
pub fn normalize_numeric_string(v: &str) -> Cow<'_, str> {
    if !v.contains(is_special) {
        return Cow::Borrowed(v);
    }

    let mut s = v.to_string();
    if s.starts_with(CURRENCY_SYMBOLS.as_slice()) {
        s.remove(0);
    }
    if s.ends_with(CURRENCY_SYMBOLS.as_slice()) {
        s.pop();
    }
    if let Some(inner) = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        s = format!("-{inner}");
    }
    s = normalize_trailing_sign(&s).into_owned();
    s = normalize_separators(&s);

    Cow::Owned(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_a_trailing_sign_to_the_front() {
        assert_eq!(normalize_numeric_string("150.00-"), "-150.00");
        assert_eq!(normalize_numeric_string("42-"), "-42");
        assert_eq!(normalize_numeric_string("42+"), "42");
    }

    #[test]
    fn reads_parentheses_as_negative() {
        assert_eq!(normalize_numeric_string("(150.00)"), "-150.00");
        assert_eq!(normalize_numeric_string("(42)"), "-42");
    }

    #[test]
    fn strips_a_leading_or_trailing_currency_symbol() {
        assert_eq!(normalize_numeric_string("$150.00"), "150.00");
        assert_eq!(normalize_numeric_string("€150,00"), "150.00");
        assert_eq!(normalize_numeric_string("150.00$"), "150.00");
    }

    #[test]
    fn resolves_thousands_separators() {
        assert_eq!(normalize_numeric_string("1,234"), "1234");
        assert_eq!(normalize_numeric_string("1,234,567"), "1234567");
        assert_eq!(normalize_numeric_string("12,345.67"), "12345.67");
    }

    #[test]
    fn resolves_a_comma_decimal_point() {
        assert_eq!(normalize_numeric_string("150,00"), "150.00");
        assert_eq!(normalize_numeric_string("12,34"), "12.34");
        assert_eq!(normalize_numeric_string("1.234,56"), "1234.56");
    }

    #[test]
    fn combines_with_a_leading_sign_or_currency() {
        assert_eq!(normalize_numeric_string("-1,234.56"), "-1234.56");
        assert_eq!(normalize_numeric_string("$1,234.56-"), "-1234.56");
        assert_eq!(normalize_numeric_string("(1,234.56)"), "-1234.56");
    }

    #[test]
    fn leaves_unremarkable_values_alone() {
        assert_eq!(normalize_numeric_string("42"), "42");
        assert_eq!(normalize_numeric_string("-42"), "-42");
        assert_eq!(normalize_numeric_string("42.5"), "42.5");
        assert_eq!(normalize_numeric_string("n/a"), "n/a");
    }

    #[test]
    fn leaves_a_lone_sign_or_non_numeric_text_alone() {
        assert_eq!(normalize_numeric_string("-"), "-");
        assert_eq!(normalize_numeric_string("+"), "+");
        assert_eq!(normalize_numeric_string("n/a-"), "-n/a");
    }
}
