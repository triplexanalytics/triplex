//! Tier 1 pseudonymization: deterministic HMAC-SHA256 tokenization of
//! declared columns, with a separate reversible mapping file that never
//! leaves the client machine. No network calls.

mod mapping;
mod recipe;
mod tokenize;

pub use mapping::{decrypt_mapping, encrypt_mapping, EncryptedMapping, MappingEntry};
pub use recipe::{load_recipe, AnonymizeColumn, AnonymizeMethod, AnonymizeRecipe};
pub use tokenize::hmac_token;

use std::collections::HashSet;
use std::path::Path;
use triplex_core::{Error, Result};

/// Mapping-equipped output is pseudonymized, not anonymous, personal data
/// under GDPR — printed by the CLI on every run and returned here so callers
/// (and tests) don't have to scrape stderr to know it happened.
pub const PSEUDONYMIZATION_WARNING: &str = "this output is pseudonymized, not anonymous, \
personal data under GDPR: the mapping file can reverse it back to the original values. \
Keep the mapping file secured and separate from the anonymized dataset.";

#[derive(serde::Serialize)]
pub struct AnonymizeReport {
    pub rows_processed: usize,
    pub columns_anonymized: Vec<String>,
    pub unique_tokens_minted: usize,
    pub warning: &'static str,
}

fn read_rows(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("csv") => {
            let sample = triplex_io::read_csv_full(path)?;
            Ok((sample.headers, sample.rows))
        }
        Some("parquet") => triplex_io::read_parquet_rows(path),
        other => Err(Error::UnsupportedExtension(format!(
            "unsupported file extension {other:?} (expected .csv or .parquet)"
        ))),
    }
}

struct ResolvedColumn<'a> {
    header_index: usize,
    declared: &'a AnonymizeColumn,
    salt: Vec<u8>,
}

fn resolve_columns<'a>(
    recipe: &'a AnonymizeRecipe,
    headers: &[String],
) -> Result<Vec<ResolvedColumn<'a>>> {
    recipe
        .columns
        .iter()
        .map(|declared| {
            let header_index = headers
                .iter()
                .position(|h| h == &declared.name)
                .ok_or_else(|| Error::ColumnNotFound(declared.name.clone()))?;
            let salt = std::env::var(&declared.salt_env)
                .map_err(|_| Error::MissingEnvVar(declared.salt_env.clone()))?
                .into_bytes();
            // AnonymizeMethod has exactly one variant today; matching (rather
            // than ignoring it) means adding a second variant forces every
            // call site that cares to be updated instead of silently
            // defaulting to token_hmac behavior.
            match declared.method {
                AnonymizeMethod::TokenHmac => {}
            }
            Ok(ResolvedColumn {
                header_index,
                declared,
                salt,
            })
        })
        .collect()
}

/// Reads `input_path` (`.csv` or `.parquet`), tokenizes every column the
/// recipe declares, writes the masked dataset to `output_path` as CSV, and
/// writes a separate reversible mapping to `mapping_out_path` — plaintext
/// CSV, or an AES-256-GCM-encrypted JSON envelope if `mapping_passphrase`
/// is given.
pub fn anonymize_file(
    input_path: &Path,
    recipe: &AnonymizeRecipe,
    output_path: &Path,
    mapping_out_path: &Path,
    mapping_passphrase: Option<&str>,
) -> Result<AnonymizeReport> {
    let (headers, mut rows) = read_rows(input_path)?;
    let resolved = resolve_columns(recipe, &headers)?;

    let mut mapping_entries = Vec::new();
    let mut seen = HashSet::new();

    for row in &mut rows {
        for col in &resolved {
            let value = &mut row[col.header_index];
            if value.is_empty() {
                continue; // nothing to mask/reveal for a missing cell
            }
            let token = tokenize::hmac_token(&col.salt, value);
            if seen.insert((col.header_index, value.clone())) {
                mapping_entries.push(mapping::MappingEntry {
                    column: col.declared.name.clone(),
                    token: token.clone(),
                    original_value: value.clone(),
                });
            }
            *value = token;
        }
    }

    triplex_io::write_csv(output_path, &headers, &rows)?;

    let mapping_bytes = mapping::mapping_to_csv(&mapping_entries)?;
    match mapping_passphrase {
        Some(passphrase) => {
            let encrypted = mapping::encrypt_mapping(&mapping_bytes, passphrase)?;
            let json = serde_json::to_vec_pretty(&encrypted)
                .map_err(|e| Error::Crypto(format!("could not serialize mapping envelope: {e}")))?;
            std::fs::write(mapping_out_path, json)?;
        }
        None => std::fs::write(mapping_out_path, mapping_bytes)?,
    }

    Ok(AnonymizeReport {
        rows_processed: rows.len(),
        columns_anonymized: resolved.iter().map(|c| c.declared.name.clone()).collect(),
        unique_tokens_minted: mapping_entries.len(),
        warning: PSEUDONYMIZATION_WARNING,
    })
}

/// Decrypts a mapping file previously produced by `anonymize_file` with a
/// passphrase, writing the recovered plaintext CSV to `output_path`. Errors
/// clearly (not a panic, not silent corruption) on a wrong passphrase or a
/// mapping file that was never encrypted (plain CSV, not a JSON envelope).
pub fn reveal_mapping(mapping_path: &Path, passphrase: &str, output_path: &Path) -> Result<()> {
    let envelope_json = std::fs::read_to_string(mapping_path)?;
    let envelope: EncryptedMapping = serde_json::from_str(&envelope_json).map_err(|e| {
        Error::Crypto(format!(
            "not an encrypted mapping file (expected a JSON envelope): {e}"
        ))
    })?;
    let plaintext = mapping::decrypt_mapping(&envelope, passphrase)?;
    std::fs::write(output_path, plaintext)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name)
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "triplex_anonymizer_test_{}_{name}",
            std::process::id()
        ))
    }

    fn recipe_for(column: &str, salt_env: &str) -> AnonymizeRecipe {
        AnonymizeRecipe {
            version: 1,
            columns: vec![AnonymizeColumn {
                name: column.to_string(),
                method: AnonymizeMethod::TokenHmac,
                salt_env: salt_env.to_string(),
            }],
        }
    }

    #[test]
    fn anonymizes_a_csv_column_leaving_others_untouched() {
        std::env::set_var("TRIPLEX_TEST_SALT_1", "test-salt");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_1");
        let output = temp_path("output_1.csv");
        let mapping_out = temp_path("mapping_1.csv");

        let report = anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            None,
        )
        .expect("anonymize should succeed");

        assert_eq!(report.rows_processed, 4);
        assert_eq!(report.columns_anonymized, vec!["name"]);
        assert_eq!(report.unique_tokens_minted, 4); // widget/gadget/sprocket/doohickey

        let masked = triplex_io::read_csv_full(&output).unwrap();
        assert_eq!(masked.headers, vec!["id", "price", "in_stock", "name"]);
        // Untouched columns keep their original values.
        assert_eq!(masked.rows[0][0], "1");
        assert_eq!(masked.rows[0][1], "9.99");
        // The anonymized column no longer contains the original value.
        assert_ne!(masked.rows[0][3], "widget");
        assert_eq!(masked.rows[0][3].len(), 64); // hex-encoded HMAC-SHA256

        let mapping = triplex_io::read_csv_full(&mapping_out).unwrap();
        assert!(mapping
            .rows
            .iter()
            .any(|r| r[0] == "name" && r[2] == "widget" && r[1] == masked.rows[0][3]));

        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();
    }

    #[test]
    fn anonymizes_a_parquet_input_the_same_way() {
        std::env::set_var("TRIPLEX_TEST_SALT_2", "test-salt");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_2");
        let output = temp_path("output_2.csv");
        let mapping_out = temp_path("mapping_2.csv");

        let report = anonymize_file(
            &fixture("sample.parquet"),
            &recipe,
            &output,
            &mapping_out,
            None,
        )
        .expect("anonymize should succeed on parquet input");

        assert_eq!(report.rows_processed, 4);
        let masked = triplex_io::read_csv_full(&output).unwrap();
        assert_ne!(masked.rows[0][3], "widget");

        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();
    }

    #[test]
    fn the_same_value_always_gets_the_same_token_within_a_run() {
        std::env::set_var("TRIPLEX_TEST_SALT_3", "test-salt");
        let recipe = recipe_for("in_stock", "TRIPLEX_TEST_SALT_3");
        let output = temp_path("output_3.csv");
        let mapping_out = temp_path("mapping_3.csv");

        anonymize_file(&fixture("sample.csv"), &recipe, &output, &mapping_out, None)
            .expect("anonymize should succeed");

        let masked = triplex_io::read_csv_full(&output).unwrap();
        // in_stock is true,false,true,true — two distinct values, so rows 0
        // and 2's tokens must match each other and differ from row 1's.
        assert_eq!(masked.rows[0][2], masked.rows[2][2]);
        assert_ne!(masked.rows[0][2], masked.rows[1][2]);

        let mapping = triplex_io::read_csv_full(&mapping_out).unwrap();
        assert_eq!(mapping.rows.len(), 2); // one entry per distinct value, not per row

        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();
    }

    #[test]
    fn an_undeclared_column_is_a_clear_error_not_a_panic() {
        std::env::set_var("TRIPLEX_TEST_SALT_4", "test-salt");
        let recipe = recipe_for("does_not_exist", "TRIPLEX_TEST_SALT_4");
        let output = temp_path("output_4.csv");
        let mapping_out = temp_path("mapping_4.csv");

        let result = anonymize_file(&fixture("sample.csv"), &recipe, &output, &mapping_out, None);
        assert!(result.is_err());
    }

    #[test]
    fn a_missing_salt_env_var_is_a_clear_error_not_a_panic() {
        std::env::remove_var("TRIPLEX_TEST_SALT_DEFINITELY_UNSET");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_DEFINITELY_UNSET");
        let output = temp_path("output_5.csv");
        let mapping_out = temp_path("mapping_5.csv");

        let result = anonymize_file(&fixture("sample.csv"), &recipe, &output, &mapping_out, None);
        assert!(result.is_err());
    }

    #[test]
    fn an_encrypted_mapping_can_be_decrypted_back_to_the_plain_csv() {
        std::env::set_var("TRIPLEX_TEST_SALT_6", "test-salt");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_6");
        let output = temp_path("output_6.csv");
        let mapping_out = temp_path("mapping_6.enc");

        anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            Some("a strong passphrase"),
        )
        .expect("anonymize should succeed");

        let envelope_json = std::fs::read_to_string(&mapping_out).unwrap();
        let envelope: EncryptedMapping = serde_json::from_str(&envelope_json).unwrap();
        let plaintext = decrypt_mapping(&envelope, "a strong passphrase").unwrap();
        let plaintext = String::from_utf8(plaintext).unwrap();
        assert!(plaintext.contains("widget"));

        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();
    }

    #[test]
    fn reveal_mapping_recovers_the_original_csv_from_an_encrypted_mapping() {
        std::env::set_var("TRIPLEX_TEST_SALT_7", "test-salt");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_7");
        let output = temp_path("output_7.csv");
        let mapping_out = temp_path("mapping_7.enc");
        let revealed = temp_path("revealed_7.csv");

        anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            Some("a strong passphrase"),
        )
        .expect("anonymize should succeed");

        reveal_mapping(&mapping_out, "a strong passphrase", &revealed)
            .expect("reveal should succeed with the correct passphrase");

        let plaintext = std::fs::read_to_string(&revealed).unwrap();
        assert!(plaintext.contains("widget"));
        assert!(plaintext.contains("column,token,original_value"));

        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();
        std::fs::remove_file(&revealed).ok();
    }

    #[test]
    fn reveal_mapping_with_the_wrong_passphrase_is_a_clear_error() {
        std::env::set_var("TRIPLEX_TEST_SALT_8", "test-salt");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_8");
        let output = temp_path("output_8.csv");
        let mapping_out = temp_path("mapping_8.enc");
        let revealed = temp_path("revealed_8.csv");

        anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            Some("correct passphrase"),
        )
        .expect("anonymize should succeed");

        let result = reveal_mapping(&mapping_out, "wrong passphrase", &revealed);
        assert!(result.is_err());

        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();
    }

    #[test]
    fn reveal_mapping_on_a_plaintext_mapping_file_is_a_clear_error() {
        std::env::set_var("TRIPLEX_TEST_SALT_9", "test-salt");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_9");
        let output = temp_path("output_9.csv");
        let mapping_out = temp_path("mapping_9.csv");
        let revealed = temp_path("revealed_9.csv");

        anonymize_file(&fixture("sample.csv"), &recipe, &output, &mapping_out, None)
            .expect("anonymize should succeed");

        let result = reveal_mapping(&mapping_out, "any passphrase", &revealed);
        assert!(result.is_err());

        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();
    }
}
