//! Tier 1 pseudonymization: deterministic masking of declared columns —
//! HMAC-SHA256 tokenization, format-preserving substitution, k-anonymity
//! bucketing, or prefix/suffix-preserving partial masking (see
//! `mechanism.rs` for how a column's declared method dispatches to one of
//! these) — with a separate reversible mapping file that never leaves the
//! client machine. No network calls.

mod bucket;
mod fpe;
mod mapping;
mod mechanism;
mod partial_mask;
mod recipe;
mod tokenize;

pub use bucket::{bucket_numeric, bucket_value};
pub use fpe::fpe_mask;
pub use mapping::{decrypt_mapping, encrypt_mapping, EncryptedMapping, MappingEntry};
pub use partial_mask::partial_mask;
pub use recipe::{
    load_recipe, verify_recipe_well_formed, AnonymizeColumn, AnonymizeMethod, AnonymizeRecipe,
};
pub use tokenize::hmac_token;
// Risk-model scoring moved to `triplex-datasetcard` — kept
// re-exported here so nothing that already does
// `use triplex_anonymizer::{RiskModel, ...}` breaks.
pub use triplex_datasetcard::{evaluate_risk_model, k_anonymity, l_diversity, t_closeness};
pub use triplex_datasetcard::{RiskAssessment, RiskModel};

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
    pub unique_masked_values_minted: usize,
    pub warning: &'static str,
}

/// The masked dataset plus its reversible mapping, both as in-memory CSV
/// bytes — the return shape `anonymize_bytes` needs since a browser/WASM
/// caller (`apps/triplex-web`) has nowhere on disk to write them.
pub struct AnonymizeBytesResult {
    pub masked_csv: Vec<u8>,
    pub mapping_csv: Vec<u8>,
    pub report: AnonymizeReport,
}

fn read_rows(path: &Path) -> Result<triplex_io::Sample> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("csv") => triplex_io::read_csv_full(path),
        Some("parquet") => triplex_io::read_parquet_rows(path),
        other => Err(Error::UnsupportedExtension(format!(
            "unsupported file extension {other:?} (expected .csv or .parquet)"
        ))),
    }
}

/// Byte-slice counterpart to `read_rows` — `.csv` only. Parquet input is out
/// of scope for `apps/triplex-web` (see `crates/triplex-io/Cargo.toml` for
/// why: its codecs need a C toolchain wasm32-unknown-unknown doesn't have).
fn read_rows_bytes(bytes: &[u8], extension: &str) -> Result<triplex_io::Sample> {
    match extension {
        "csv" => triplex_io::read_csv_full_bytes(bytes),
        other => Err(Error::UnsupportedExtension(format!(
            "unsupported file extension {other:?} (expected .csv — .parquet input isn't \
             supported in the browser)"
        ))),
    }
}

struct ResolvedColumn<'a> {
    header_index: usize,
    declared: &'a AnonymizeColumn,
    mechanism: Box<dyn mechanism::Mechanism>,
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
            Ok(ResolvedColumn {
                header_index,
                declared,
                mechanism: mechanism::build_mechanism(declared, &salt),
            })
        })
        .collect()
}

/// Like `resolve_columns`, but takes each column's salt directly from
/// `salts` (keyed by declared column name) instead of reading an OS env
/// var — the browser/WASM path (`apps/triplex-web`) has no environment to
/// read from; the salt comes from what the user typed into the page.
fn resolve_columns_with_salts<'a>(
    recipe: &'a AnonymizeRecipe,
    headers: &[String],
    salts: &std::collections::HashMap<String, String>,
) -> Result<Vec<ResolvedColumn<'a>>> {
    recipe
        .columns
        .iter()
        .map(|declared| {
            let header_index = headers
                .iter()
                .position(|h| h == &declared.name)
                .ok_or_else(|| Error::ColumnNotFound(declared.name.clone()))?;
            let salt = salts
                .get(&declared.name)
                .ok_or_else(|| Error::MissingSalt(declared.name.clone()))?
                .clone()
                .into_bytes();
            Ok(ResolvedColumn {
                header_index,
                declared,
                mechanism: mechanism::build_mechanism(declared, &salt),
            })
        })
        .collect()
}

/// The masking loop shared by every entry point: runs each resolved
/// column's `Mechanism` over its values in place, minting one mapping entry
/// per distinct (column, original_value) pair. Pure — no I/O — so it runs
/// the same way whether the `Sample` came from a local file or from
/// browser bytes. Oblivious to *which* mechanism a column uses — adding a
/// fifth one (see `mechanism.rs`) never touches this loop.
fn mask_rows(resolved: &[ResolvedColumn], sample: triplex_io::Sample) -> (triplex_io::Sample, Vec<mapping::MappingEntry>) {
    let (headers, mut rows) = (sample.headers, sample.rows);
    let mut mapping_entries = Vec::new();
    let mut seen = HashSet::new();

    for row in &mut rows {
        for col in resolved {
            let value = &mut row[col.header_index];
            if value.is_empty() {
                continue; // nothing to mask/reveal for a missing cell
            }
            let masked = col.mechanism.mask(value);
            if seen.insert((col.header_index, value.clone())) {
                mapping_entries.push(mapping::MappingEntry {
                    column: col.declared.name.clone(),
                    masked_value: masked.clone(),
                    original_value: value.clone(),
                });
            }
            *value = masked;
        }
    }

    (triplex_io::Sample { headers, rows }, mapping_entries)
}

/// Reads `input_path` (`.csv` or `.parquet`), masks every column the
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
    let sample = read_rows(input_path)?;
    let resolved = resolve_columns(recipe, &sample.headers)?;
    let (masked, mapping_entries) = mask_rows(&resolved, sample);

    triplex_io::write_csv(output_path, &masked.headers, &masked.rows)?;

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
        rows_processed: masked.rows.len(),
        columns_anonymized: resolved.iter().map(|c| c.declared.name.clone()).collect(),
        unique_masked_values_minted: mapping_entries.len(),
        warning: PSEUDONYMIZATION_WARNING,
    })
}

/// Byte-slice counterpart to `anonymize_file`, for a caller with no
/// filesystem (`apps/triplex-web`'s browser/WASM path): takes the input
/// file's bytes and returns the masked CSV and mapping CSV as bytes instead
/// of writing either to disk. `salts` maps declared column name to salt —
/// see `resolve_columns_with_salts` for why this replaces the CLI's
/// env-var lookup. No passphrase encryption here; the mapping is offered as
/// a plaintext download, same trust boundary as any other client-side-only
/// output (still `PSEUDONYMIZATION_WARNING`'s responsibility to convey).
pub fn anonymize_bytes(
    bytes: &[u8],
    extension: &str,
    recipe: &AnonymizeRecipe,
    salts: &std::collections::HashMap<String, String>,
) -> Result<AnonymizeBytesResult> {
    let sample = read_rows_bytes(bytes, extension)?;
    let resolved = resolve_columns_with_salts(recipe, &sample.headers, salts)?;
    let (masked, mapping_entries) = mask_rows(&resolved, sample);

    let masked_csv = triplex_io::write_csv_bytes(&masked.headers, &masked.rows)?;
    let mapping_csv = mapping::mapping_to_csv(&mapping_entries)?;

    Ok(AnonymizeBytesResult {
        masked_csv,
        report: AnonymizeReport {
            rows_processed: masked.rows.len(),
            columns_anonymized: resolved.iter().map(|c| c.declared.name.clone()).collect(),
            unique_masked_values_minted: mapping_entries.len(),
            warning: PSEUDONYMIZATION_WARNING,
        },
        mapping_csv,
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
        recipe_with_method(column, AnonymizeMethod::TokenHmac, salt_env)
    }

    fn recipe_with_method(
        column: &str,
        method: AnonymizeMethod,
        salt_env: &str,
    ) -> AnonymizeRecipe {
        AnonymizeRecipe {
            version: 1,
            columns: vec![AnonymizeColumn {
                name: column.to_string(),
                method,
                salt_env: salt_env.to_string(),
                bucket_width: 10,
                keep_prefix: 0,
                keep_suffix: 4,
            }],
        }
    }

    #[test]
    fn anonymize_bytes_masks_a_column_and_returns_a_matching_mapping() {
        let recipe = recipe_for("name", "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string());

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts)
            .expect("anonymize_bytes should succeed");

        assert_eq!(result.report.rows_processed, 4);
        assert_eq!(result.report.columns_anonymized, vec!["name"]);
        assert_eq!(result.report.unique_masked_values_minted, 4);

        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();
        assert_ne!(masked.rows[0][3], "widget");
        assert_eq!(masked.rows[0][3].len(), 64);

        let mapping = triplex_io::read_csv_full_bytes(&result.mapping_csv).unwrap();
        assert!(mapping
            .rows
            .iter()
            .any(|r| r[0] == "name" && r[2] == "widget" && r[1] == masked.rows[0][3]));
    }

    #[test]
    fn anonymize_bytes_with_fpe_preserves_length_and_character_class() {
        let recipe = recipe_with_method("name", AnonymizeMethod::Fpe, "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string());

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts)
            .expect("anonymize_bytes should succeed");
        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        assert_ne!(masked.rows[0][3], "widget");
        assert_eq!(masked.rows[0][3].len(), "widget".len());
        assert!(masked.rows[0][3].chars().all(|c| c.is_ascii_lowercase()));
    }

    #[test]
    fn anonymize_bytes_with_bucket_generalizes_a_numeric_column() {
        let recipe = recipe_with_method("id", AnonymizeMethod::Bucket, "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("id".to_string(), "unused".to_string()); // Bucket ignores the salt

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts)
            .expect("anonymize_bytes should succeed");
        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        // ids 1..4 (recipe_with_method's default bucket_width: 10) all fall
        // in the same "0-9" bucket.
        assert!(masked.rows.iter().all(|r| r[0] == "0-9"));
    }

    #[test]
    fn anonymize_bytes_with_partial_mask_keeps_only_the_configured_suffix() {
        let recipe = recipe_with_method(
            "name",
            AnonymizeMethod::PartialMask,
            "unused_in_the_bytes_path",
        );
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "unused".to_string()); // PartialMask ignores the salt

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts)
            .expect("anonymize_bytes should succeed");
        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        // recipe_with_method's default keep_suffix: 4 — "widget" keeps "dget".
        assert_eq!(masked.rows[0][3], "**dget");
    }

    #[test]
    fn anonymize_bytes_matches_anonymize_file_for_the_same_input_and_salt() {
        std::env::set_var("TRIPLEX_TEST_SALT_BYTES_PARITY", "test-salt");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_BYTES_PARITY");
        let output = temp_path("output_bytes_parity.csv");
        let mapping_out = temp_path("mapping_bytes_parity.csv");

        anonymize_file(&fixture("sample.csv"), &recipe, &output, &mapping_out, None)
            .expect("anonymize_file should succeed");
        let via_path = triplex_io::read_csv_full(&output).unwrap();
        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();

        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string());
        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts).unwrap();
        let via_bytes = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        assert_eq!(via_bytes.rows, via_path.rows, "the same salt must mint the same tokens regardless of delivery surface");
    }

    #[test]
    fn anonymize_bytes_with_no_salt_for_a_declared_column_is_a_clear_error_not_a_panic() {
        let recipe = recipe_for("name", "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let salts = std::collections::HashMap::new(); // no "name" entry

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts);
        match result {
            Err(e) => assert_eq!(e.code(), "MISSING_SALT"),
            Ok(_) => panic!("expected an error for a column with no supplied salt"),
        }
    }

    #[test]
    fn anonymize_bytes_rejects_parquet_with_a_clear_error_not_a_panic() {
        let recipe = recipe_for("name", "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.parquet")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string());

        let result = anonymize_bytes(&bytes, "parquet", &recipe, &salts);
        assert!(result.is_err(), "parquet input isn't supported via bytes");
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
        assert_eq!(report.unique_masked_values_minted, 4); // widget/gadget/sprocket/doohickey

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
        assert!(plaintext.contains("column,masked_value,original_value"));

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
