//! Sensitive-column semantic type detection, layered on top of the primitive
//! `ColumnType`. Rule-based (regex/checksum + header-keyword matching) only —
//! the design notes's "Sensitive-column / PII detection
//! roadmap" fixes this as the year-1 method, no ML/NER classifier.
//!
//! No country-specific pattern library (proper phone-number or postal-code
//! validation per country, the way `libphonenumber` does it) exists in this
//! repo yet — `regex` here is the same general-purpose crate `contract.rs`
//! already depends on for user-declared `pattern` fields, not a curated set
//! of per-country formats. `Phone` below is deliberately just an
//! E.164-shaped heuristic for that reason; building out real per-country
//! coverage is a separate, undecided scope.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticType {
    /// Digit-only string an identifier, not a quantity — e.g. `"007"` or
    /// `"00000000000001"`. `ColumnType::String` already keeps its padding
    /// intact (see `has_meaningless_leading_zero`); this just names *why*.
    NumericId,
    Iban,
    Bic,
    Email,
    /// E.164-shaped only (leading `+`, 7-15 digits once separators are
    /// stripped) — not validated per country. A real country-aware phone
    /// library (return codes, area-code lengths, national vs. international
    /// formats) is real scope that hasn't been decided yet; see the module
    /// doc comment.
    Phone,
    /// Luhn (mod-10) checksum over 13-19 digits — covers every major card
    /// network's number length, not just one issuer's.
    CreditCard,
    /// `aa:bb:cc:dd:ee:ff` or `aa-bb-cc-dd-ee-ff`, six hex-byte groups. Named
    /// in Tonic's own generator list (the design notes)
    /// as a distinct sensitive type worth its own generator.
    MacAddress,
    /// IPv4 (four dot-separated octets, each 0-255) or a permissive IPv6
    /// shape check (2-8 colon-separated hex groups, at most one `::`) — not
    /// a full RFC 4291 validator (e.g. doesn't reject a `::` that wouldn't
    /// actually shorten anything). Good enough to flag the column; a
    /// borderline-invalid address slipping through costs nothing here since
    /// this only decides *how sensitive to treat this column*, not whether
    /// to route real network traffic.
    IpAddress,
    /// Header-keyword only, deliberately broad instead of one variant per
    /// document type (passport/driver's-license/tax-id/SSN all differ by
    /// country with no universal checksum) — see the module doc comment on
    /// why no per-country validator library exists here.
    NationalId,
    /// Header-keyword only (`dob`, `birth`, ...) — no content shape to check
    /// generically, dates come in too many formats, but the header signal
    /// alone is worth surfacing since date of birth is a classic
    /// quasi-identifier.
    DateOfBirth,
    /// Header-keyword only (`password`, `secret`, `api_key`, `token`, ...).
    /// Checked before every content-based type in `detect_semantic_type`:
    /// a column deliberately named for a credential should never lose that
    /// label to an incidental content-shape coincidence (e.g. a password
    /// that happens to parse as `Email`-shaped).
    CredentialSecret,
    Address,
    Location,
    /// An `Integer` column whose non-empty values are almost all unique
    /// (≥95% distinct relative to row count) — probably a row identifier
    /// (a sequential ID, an order or customer number), not a quantity
    /// anyone means to add, average, or bucket. Cardinality-only heuristic,
    /// deliberately no header-keyword gate — see
    /// the design notes. A dismissible hint, not an
    /// error: the column stays typed `Integer`, and a small file can still
    /// false-positive (few rows, all-distinct values) or a genuinely
    /// high-cardinality quantity column can still trip it.
    LikelyIdentifier,
}

fn header_matches(name: &str, keywords: &[&str]) -> bool {
    let lower = name.to_ascii_lowercase();
    keywords.iter().any(|k| lower.contains(k))
}

const ADDRESS_KEYWORDS: &[&str] = &["address", "addr", "street", "straat", "strasse"];
const LOCATION_KEYWORDS: &[&str] = &[
    "city", "town", "location", "region", "state", "province", "country", "postal", "zip",
    "postcode",
];
const BIC_KEYWORDS: &[&str] = &["bic", "swift"];
const PHONE_KEYWORDS: &[&str] = &["phone", "mobile", "tel", "fax", "gsm"];
const NATIONAL_ID_KEYWORDS: &[&str] = &[
    "ssn",
    "social_security",
    "social security",
    "passport",
    "national_id",
    "national id",
    "driver_license",
    "driver's license",
    "drivers_license",
    "tax_id",
    "tax id",
];
const DOB_KEYWORDS: &[&str] = &["dob", "date_of_birth", "date of birth", "birthdate", "birthday"];
const CREDENTIAL_KEYWORDS: &[&str] = &[
    "password", "passwd", "secret", "api_key", "apikey", "access_token", "auth_token",
];

/// HTML5's pragmatic email pattern (`local@domain.tld`) — not full RFC 5322,
/// which accepts addresses no real-world column would ever contain.
fn email_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^[a-z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$",
        )
        .unwrap()
    })
}

/// A whole-number-looking string with a padding zero, e.g. `"007"` or
/// `"00000000000001"` — mirrors `has_meaningless_leading_zero` in `lib.rs`
/// but stands alone since this module has no access to the columns that
/// already parsed as `i64`.
fn is_numeric_id(v: &str) -> bool {
    let digits = v.strip_prefix('-').unwrap_or(v);
    digits.len() > 1 && digits.starts_with('0') && digits.chars().all(|c| c.is_ascii_digit())
}

/// ISO 7064 mod-97-10 IBAN checksum: move the first 4 characters to the end,
/// map letters to numbers (A=10 .. Z=35), and the result mod 97 must be 1.
/// Structural shape (2-letter country + 2-digit check + alphanumeric BBAN,
/// 15-34 chars total) is checked first so a random short alnum string can't
/// even reach the checksum.
fn is_valid_iban(v: &str) -> bool {
    let s: String = v.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() < 15 || s.len() > 34 || !s.is_ascii() {
        return false;
    }
    let bytes = s.as_bytes();
    let country_ok = bytes[0..2].iter().all(|b| b.is_ascii_alphabetic());
    let check_digits_ok = bytes[2..4].iter().all(|b| b.is_ascii_digit());
    let bban_ok = bytes[4..].iter().all(|b| b.is_ascii_alphanumeric());
    if !country_ok || !check_digits_ok || !bban_ok {
        return false;
    }

    let rearranged = format!("{}{}", &s[4..], &s[..4]).to_ascii_uppercase();
    let mut remainder: u64 = 0;
    for c in rearranged.chars() {
        let value = if c.is_ascii_digit() {
            c as u64 - '0' as u64
        } else {
            c as u64 - 'A' as u64 + 10
        };
        for digit in value.to_string().chars() {
            remainder = (remainder * 10 + digit.to_digit(10).unwrap() as u64) % 97;
        }
    }
    remainder == 1
}

/// SWIFT/BIC shape: 4-letter bank code + 2-letter country code + 2
/// alphanumeric location code, plus an optional 3-alphanumeric branch code
/// (8 or 11 characters total). This shape alone is too weak a signal on its
/// own (it also matches plenty of unrelated 8-char codes), so callers gate
/// it on a header-keyword match too.
fn is_bic_shaped(v: &str) -> bool {
    let s = v.trim();
    if !s.is_ascii() {
        return false;
    }
    let bytes = s.as_bytes();
    matches!(bytes.len(), 8 | 11)
        && bytes[0..6].iter().all(|b| b.is_ascii_alphabetic())
        && bytes[6..8].iter().all(|b| b.is_ascii_alphanumeric())
        && bytes[8..].iter().all(|b| b.is_ascii_alphanumeric())
}

/// E.164-shaped: an optional leading `+`, then 7-15 digits once common
/// separators (space, dash, dot, parens) are stripped — see the module doc
/// comment for why this isn't a real per-country validator.
fn is_phone_shaped(v: &str) -> bool {
    let trimmed = v.trim();
    let rest = trimmed.strip_prefix('+').unwrap_or(trimmed);
    if rest.is_empty()
        || !rest
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ' ' | '-' | '.' | '(' | ')'))
    {
        return false;
    }
    let digit_count = rest.chars().filter(|c| c.is_ascii_digit()).count();
    (7..=15).contains(&digit_count)
}

/// Luhn (mod-10) checksum over 13-19 digits (common separators stripped
/// first) — the standard check digit algorithm shared by every major card
/// network, not one issuer's format.
fn is_valid_credit_card(v: &str) -> bool {
    let digits: String = v.chars().filter(|c| !matches!(c, ' ' | '-')).collect();
    if digits.len() < 13 || digits.len() > 19 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let sum: u32 = digits
        .chars()
        .rev()
        .enumerate()
        .map(|(i, c)| {
            let d = c.to_digit(10).unwrap();
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// `aa:bb:cc:dd:ee:ff` or `aa-bb-cc-dd-ee-ff` — six colon- or
/// hyphen-separated hex-byte groups, one separator style used consistently
/// (real MAC addresses aren't written with a mix of the two).
fn is_mac_address_shaped(v: &str) -> bool {
    let s = v.trim();
    for sep in [':', '-'] {
        let groups: Vec<&str> = s.split(sep).collect();
        if groups.len() == 6
            && groups
                .iter()
                .all(|g| g.len() == 2 && g.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return true;
        }
    }
    false
}

fn is_ipv4_shaped(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.chars().all(|c| c.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|n| n <= 255)
        })
}

/// Permissive shape check, not a full RFC 4291 validator — see the
/// `IpAddress` variant doc comment for what that trade-off costs (nothing,
/// here: this only decides how sensitive to treat a column).
fn is_ipv6_shaped(v: &str) -> bool {
    let s = v.trim();
    if s.matches("::").count() > 1 {
        return false;
    }
    let groups: Vec<&str> = s.split(':').filter(|g| !g.is_empty()).collect();
    (2..=8).contains(&groups.len())
        && groups
            .iter()
            .all(|g| g.len() <= 4 && g.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Detects a sensitive-column semantic type for a `ColumnType::String`
/// column. `non_empty` is empty for Parquet (footer-only stats carry no raw
/// values), which still lets the header-keyword-only types (Address,
/// Location) through — every other type needs at least one real value and
/// requires *every* non-empty value to match, the same all-or-nothing bar
/// `infer_column_type` uses.
pub(crate) fn detect_semantic_type(name: &str, non_empty: &[&str]) -> Option<SemanticType> {
    // Checked first, ahead of every content-based type: a column
    // deliberately named for a credential should never lose that label to
    // an incidental content-shape coincidence.
    if header_matches(name, CREDENTIAL_KEYWORDS) {
        return Some(SemanticType::CredentialSecret);
    }
    if !non_empty.is_empty() {
        if non_empty.iter().all(|v| is_numeric_id(v)) {
            return Some(SemanticType::NumericId);
        }
        if non_empty.iter().all(|v| is_valid_iban(v)) {
            return Some(SemanticType::Iban);
        }
        if non_empty.iter().all(|v| email_regex().is_match(v)) {
            return Some(SemanticType::Email);
        }
        if non_empty.iter().all(|v| is_valid_credit_card(v)) {
            return Some(SemanticType::CreditCard);
        }
        if non_empty.iter().all(|v| is_mac_address_shaped(v)) {
            return Some(SemanticType::MacAddress);
        }
        if non_empty
            .iter()
            .all(|v| is_ipv4_shaped(v) || is_ipv6_shaped(v))
        {
            return Some(SemanticType::IpAddress);
        }
        if header_matches(name, BIC_KEYWORDS) && non_empty.iter().all(|v| is_bic_shaped(v)) {
            return Some(SemanticType::Bic);
        }
        if header_matches(name, PHONE_KEYWORDS) && non_empty.iter().all(|v| is_phone_shaped(v)) {
            return Some(SemanticType::Phone);
        }
    }
    if header_matches(name, NATIONAL_ID_KEYWORDS) {
        return Some(SemanticType::NationalId);
    }
    if header_matches(name, DOB_KEYWORDS) {
        return Some(SemanticType::DateOfBirth);
    }
    if header_matches(name, ADDRESS_KEYWORDS) {
        return Some(SemanticType::Address);
    }
    if header_matches(name, LOCATION_KEYWORDS) {
        return Some(SemanticType::Location);
    }
    None
}

/// Fraction of unique non-empty values above which an `Integer` column
/// reads as an identifier rather than a quantity — see `LikelyIdentifier`.
const LIKELY_IDENTIFIER_UNIQUE_RATIO: f64 = 0.95;

/// Detects `SemanticType::LikelyIdentifier` for a `ColumnType::Integer`
/// column. `non_empty` empty (Parquet footer-only stats carry no raw
/// values) means nothing to check, same as `detect_semantic_type`.
pub(crate) fn detect_integer_semantic_type(non_empty: &[&str]) -> Option<SemanticType> {
    if non_empty.len() < 2 {
        return None;
    }
    let distinct = non_empty
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    if distinct as f64 / non_empty.len() as f64 >= LIKELY_IDENTIFIER_UNIQUE_RATIO {
        Some(SemanticType::LikelyIdentifier)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_zero_padded_numeric_ids() {
        let values = ["00000000000001", "000000002"];
        for v in values {
            assert_eq!(
                detect_semantic_type("account", &[v]),
                Some(SemanticType::NumericId),
                "expected {v} to be detected as a numeric id",
            );
        }
    }

    #[test]
    fn detects_a_valid_iban_by_checksum_alone() {
        // NL91ABNA0417164300 — the standard IBAN checksum worked example.
        assert_eq!(
            detect_semantic_type("account_number", &["NL91ABNA0417164300"]),
            Some(SemanticType::Iban),
        );
    }

    #[test]
    fn rejects_an_iban_shaped_string_with_a_bad_checksum() {
        assert_eq!(
            detect_semantic_type("account_number", &["NL91ABNA0417164301"]),
            None,
        );
    }

    #[test]
    fn detects_bic_only_with_a_matching_header() {
        assert_eq!(
            detect_semantic_type("swift_bic", &["ABNANL2A"]),
            Some(SemanticType::Bic),
        );
        assert_eq!(
            detect_semantic_type("bank_code", &["ABNANL2A"]),
            None,
            "bic-shaped content without a bic/swift header should not be tagged",
        );
    }

    #[test]
    fn detects_email_by_content_alone() {
        assert_eq!(
            detect_semantic_type("contact", &["alice@example.com", "bob@example.co.uk"]),
            Some(SemanticType::Email),
        );
        assert_eq!(detect_semantic_type("contact", &["not-an-email"]), None);
    }

    #[test]
    fn detects_phone_only_with_a_matching_header() {
        assert_eq!(
            detect_semantic_type("mobile", &["+31 6 1234 5678", "020-123-4567"]),
            Some(SemanticType::Phone),
        );
        assert_eq!(
            detect_semantic_type("account_number", &["+31 6 1234 5678"]),
            None,
            "phone-shaped digits without a phone/mobile/tel header should not be tagged",
        );
    }

    #[test]
    fn detects_address_and_location_by_header_only() {
        assert_eq!(
            detect_semantic_type("shipping_address", &[]),
            Some(SemanticType::Address),
        );
        assert_eq!(
            detect_semantic_type("city", &[]),
            Some(SemanticType::Location)
        );
    }

    #[test]
    fn plain_strings_are_not_tagged() {
        assert_eq!(detect_semantic_type("description", &["hello world"]), None);
    }

    #[test]
    fn detects_a_valid_credit_card_by_luhn_checksum_alone() {
        // 4111 1111 1111 1111 — the standard Visa test number.
        assert_eq!(
            detect_semantic_type("card_number", &["4111111111111111", "4111-1111-1111-1111"]),
            Some(SemanticType::CreditCard),
        );
    }

    #[test]
    fn rejects_a_credit_card_shaped_string_with_a_bad_checksum() {
        assert_eq!(
            detect_semantic_type("card_number", &["4111111111111112"]),
            None,
        );
    }

    #[test]
    fn detects_mac_addresses_by_content_alone() {
        assert_eq!(
            detect_semantic_type("device", &["aa:bb:cc:dd:ee:ff", "00-11-22-33-44-55"]),
            Some(SemanticType::MacAddress),
        );
        assert_eq!(
            detect_semantic_type("device", &["aa:bb:cc:dd:ee:zz"]),
            None,
            "non-hex characters should not be tagged as a MAC address",
        );
    }

    #[test]
    fn detects_ipv4_and_ipv6_addresses_by_content_alone() {
        assert_eq!(
            detect_semantic_type("client_ip", &["192.168.1.1", "8.8.8.8"]),
            Some(SemanticType::IpAddress),
        );
        assert_eq!(
            detect_semantic_type("client_ip", &["2001:0db8:85a3:0000:0000:8a2e:0370:7334"]),
            Some(SemanticType::IpAddress),
        );
        assert_eq!(
            detect_semantic_type("client_ip", &["192.168.1.999"]),
            None,
            "an out-of-range octet should not be tagged as an IPv4 address",
        );
    }

    #[test]
    fn detects_national_id_and_date_of_birth_by_header_only() {
        assert_eq!(
            detect_semantic_type("passport_number", &[]),
            Some(SemanticType::NationalId),
        );
        assert_eq!(detect_semantic_type("ssn", &[]), Some(SemanticType::NationalId));
        assert_eq!(
            detect_semantic_type("date_of_birth", &[]),
            Some(SemanticType::DateOfBirth),
        );
    }

    #[test]
    fn detects_credential_secrets_by_header_alone_even_over_matching_content() {
        assert_eq!(
            detect_semantic_type("api_key", &[]),
            Some(SemanticType::CredentialSecret),
        );
        // A "password" column whose values happen to be email-shaped should
        // still be tagged as a credential, not lose that label to the
        // content-based Email check.
        assert_eq!(
            detect_semantic_type("password", &["alice@example.com"]),
            Some(SemanticType::CredentialSecret),
        );
    }

    #[test]
    fn flags_a_near_all_unique_integer_column_as_likely_identifier() {
        let values: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
        let refs: Vec<&str> = values.iter().map(String::as_str).collect();
        assert_eq!(
            detect_integer_semantic_type(&refs),
            Some(SemanticType::LikelyIdentifier),
        );
    }

    #[test]
    fn does_not_flag_a_low_cardinality_integer_column() {
        let values = ["30", "31", "30", "32", "30", "31", "30", "30", "31", "30"];
        assert_eq!(detect_integer_semantic_type(&values), None);
    }

    #[test]
    fn does_not_flag_a_single_value_integer_column() {
        assert_eq!(detect_integer_semantic_type(&["1"]), None);
    }
}
