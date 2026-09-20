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

pub use bucket::{bucket_numeric, bucket_value, generalize_string_prefix};
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
    /// Columns declared with `AnonymizeMethod::Drop` — excluded from the
    /// output entirely, not masked in place, so tracked separately from
    /// `columns_anonymized` rather than folded into it.
    pub columns_dropped: Vec<String>,
    pub unique_masked_values_minted: usize,
    /// The audit-JSON risk report the design notes's Tier 1 row promises
    /// ("audit JSON reporting which risk model(s) the output satisfies") —
    /// see `assess_bucket_risk`. `None` when the recipe declares no
    /// `AnonymizeMethod::Bucket` column, since there's no quasi-identifier
    /// to score.
    pub risk_assessment: Option<RiskAssessment>,
    /// How many input rows `exclude_rows` actually removed before anything
    /// else ran — a caller-chosen exclusion (e.g. apps/triplex-web's row
    /// filter, "remove these rows from the output"), not a recipe
    /// declaration, so it has no `AnonymizeColumn`-style name list the way
    /// `columns_dropped` does.
    pub rows_excluded: usize,
    pub warning: &'static str,
}

/// k-anonymity over every `AnonymizeMethod::Bucket` column's own masked
/// (generalized) values, composed into one equivalence-class key if there's
/// more than one — `Bucket`, by definition, is what makes a column a
/// quasi-identifier here, so this needs no separate recipe declaration (see
/// the decision to keep v1 to auto k-anonymity only, not a
/// selectable model + declared sensitive column). Reads `masked`, not the
/// original sample: the risk being reported is about the *output*, the same
/// composite an attacker would actually see. `.expect(...)`, not `?`:
/// `RiskModel::KAnonymity` is the one `evaluate_risk_model` arm that always
/// computes directly from the values given (see that function) — it cannot
/// error the way `DeltaPresence`/`KMap` do.
fn assess_bucket_risk(
    resolved: &[ResolvedColumn],
    masked: &triplex_io::Sample,
) -> Option<RiskAssessment> {
    let bucket_columns: Vec<Vec<String>> = resolved
        .iter()
        .filter(|c| c.declared.method == AnonymizeMethod::Bucket)
        .map(|c| {
            masked
                .rows
                .iter()
                .map(|row| row[c.header_index].clone())
                .collect()
        })
        .collect();
    if bucket_columns.is_empty() {
        return None;
    }
    let composite = triplex_datasetcard::compose_quasi_identifiers(&bucket_columns);
    Some(
        evaluate_risk_model(RiskModel::KAnonymity, &composite, &[])
            .expect("RiskModel::KAnonymity never errors"),
    )
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

/// Splits a recipe's declared columns into the ones that get masked in
/// place (`ResolvedColumn`s, needing a real salt) and the ones declared
/// `AnonymizeMethod::Drop` (just their header index, collected into the
/// returned set — `Drop` needs no salt and no `Mechanism`, since it never
/// touches a cell value at all; see `drop_columns`, which does the actual
/// exclusion after masking runs).
///
/// `salt_for` resolves one non-`Drop` column's salt bytes — generic over
/// *where* a salt comes from so this runs the same way regardless of
/// delivery surface: the CLI reads an OS env var named by `salt_env`, while
/// `apps/triplex-web`'s browser/WASM path (no OS environment to read from)
/// looks a salt up by column name in a map the page already built from what
/// the user typed in. A future delivery surface (e.g. Python bindings) is
/// free to resolve salts its own way without this function changing at all.
fn resolve_columns<'a>(
    recipe: &'a AnonymizeRecipe,
    headers: &[String],
    salt_for: impl Fn(&AnonymizeColumn) -> Result<Vec<u8>>,
) -> Result<(Vec<ResolvedColumn<'a>>, HashSet<usize>)> {
    let mut resolved = Vec::new();
    let mut drop_indices = HashSet::new();
    for declared in &recipe.columns {
        let header_index = headers
            .iter()
            .position(|h| h == &declared.name)
            .ok_or_else(|| Error::ColumnNotFound(declared.name.clone()))?;
        if declared.method == AnonymizeMethod::Drop {
            drop_indices.insert(header_index);
            continue;
        }
        let salt = salt_for(declared)?;
        resolved.push(ResolvedColumn {
            header_index,
            declared,
            mechanism: mechanism::build_mechanism(declared, &salt),
        });
    }
    Ok((resolved, drop_indices))
}

/// `resolve_columns`'s env-var salt source — the CLI/native path. The salt
/// itself is never a CLI argument, so it never lands in shell history or a
/// process listing.
fn salt_from_env(declared: &AnonymizeColumn) -> Result<Vec<u8>> {
    std::env::var(&declared.salt_env)
        .map(String::into_bytes)
        .map_err(|_| Error::MissingEnvVar(declared.salt_env.clone()))
}

/// `resolve_columns`'s explicit-map salt source — the browser/WASM path
/// (`apps/triplex-web`), keyed by declared column name since there's no OS
/// environment to read a `salt_env` from; the salt comes from what the user
/// typed into the page.
fn salt_from_map<'a>(
    salts: &'a std::collections::HashMap<String, String>,
) -> impl Fn(&AnonymizeColumn) -> Result<Vec<u8>> + 'a {
    move |declared| {
        salts
            .get(&declared.name)
            .ok_or_else(|| Error::MissingSalt(declared.name.clone()))
            .map(|s| s.clone().into_bytes())
    }
}

/// Removes every column at a `drop_indices` header index from both the
/// header row and every data row — `AnonymizeMethod::Drop`'s actual effect:
/// the column doesn't appear in the output at all, unlike every other
/// method, which masks a column's values in place but keeps the column
/// itself. Its original values are never even examined by `mask_rows`
/// (`Drop` columns never enter `resolved`), so nothing about them reaches
/// this far except the fact that they exist — same trust boundary as any
/// other value that never leaves the client machine.
fn drop_columns(sample: triplex_io::Sample, drop_indices: &HashSet<usize>) -> triplex_io::Sample {
    if drop_indices.is_empty() {
        return sample;
    }
    let keep = |row: Vec<String>| -> Vec<String> {
        row.into_iter()
            .enumerate()
            .filter(|(i, _)| !drop_indices.contains(i))
            .map(|(_, v)| v)
            .collect()
    };
    triplex_io::Sample {
        headers: keep(sample.headers),
        rows: sample.rows.into_iter().map(keep).collect(),
    }
}

/// Drops every row at an `excluded_row_indices` position (0-based, matching
/// the input file's row order) before anything else runs — the row analog
/// of `drop_columns`, except rows are excluded by the *caller's* own choice
/// (e.g. apps/triplex-web's row-filter brush, "remove these rows from the
/// output") rather than a recipe declaration: there's no per-row equivalent
/// of a column's declared `AnonymizeMethod`, so this takes a plain index set
/// instead of deriving one from `recipe`. Applied before
/// `resolve_columns`/`mask_rows`/`assess_bucket_risk`, so an excluded row's
/// values never count toward `unique_masked_values_minted`, the
/// k-anonymity assessment, or the output at all — same as if it had never
/// been in the input. Returns how many rows were actually removed (an
/// out-of-range index in `excluded_row_indices` removes nothing), for
/// `AnonymizeReport::rows_excluded`.
fn exclude_rows(
    sample: triplex_io::Sample,
    excluded_row_indices: &HashSet<usize>,
) -> (triplex_io::Sample, usize) {
    if excluded_row_indices.is_empty() {
        return (sample, 0);
    }
    let before = sample.rows.len();
    let rows: Vec<_> = sample
        .rows
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !excluded_row_indices.contains(i))
        .map(|(_, row)| row)
        .collect();
    let removed = before - rows.len();
    (
        triplex_io::Sample {
            headers: sample.headers,
            rows,
        },
        removed,
    )
}

/// The masking loop shared by every entry point: runs each resolved
/// column's `Mechanism` over its values in place, minting one mapping entry
/// per distinct (column, original_value) pair. Pure — no I/O — so it runs
/// the same way whether the `Sample` came from a local file or from
/// browser bytes. Oblivious to *which* mechanism a column uses — adding a
/// fifth one (see `mechanism.rs`) never touches this loop.
fn mask_rows(
    resolved: &[ResolvedColumn],
    sample: triplex_io::Sample,
) -> (triplex_io::Sample, Vec<mapping::MappingEntry>) {
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

const PROFILE_SAMPLE_ROWS: usize = 1000;

/// Writes the `baseline.profile.json` artifact the design notes
/// the design notes describes Anonymizer as auto-minting: the masked
/// output's own schema (re-inferred from what was actually written, not the
/// input) plus whatever risk assessment this run computed.
/// `triplex-driftguard`'s `DriftProfile` is the intended consumer -- this
/// crate has no dependency on it and constructs the same JSON shape by
/// convention, not a shared Rust type, so DriftGuard's tier-independence
/// doesn't require a dependency back onto Anonymizer.
fn mint_profile(
    output_path: &Path,
    risk_assessment: Option<RiskAssessment>,
    profile_out_path: &Path,
) -> Result<()> {
    #[derive(serde::Serialize)]
    struct Profile {
        schema: triplex_schemacast::SchemaContract,
        #[serde(skip_serializing_if = "Option::is_none")]
        risk_assessment: Option<RiskAssessment>,
    }
    let schema = triplex_schemacast::infer_schema_from_path(output_path, PROFILE_SAMPLE_ROWS)?;
    let json = serde_json::to_vec_pretty(&Profile {
        schema,
        risk_assessment,
    })
    .map_err(|e| Error::Validation(format!("could not serialize drift profile: {e}")))?;
    std::fs::write(profile_out_path, json)?;
    Ok(())
}

/// Reads `input_path` (`.csv` or `.parquet`), excludes `excluded_row_indices`
/// (see `exclude_rows`), masks every column the recipe declares, writes the
/// masked dataset to `output_path` as CSV, and writes a separate reversible
/// mapping to `mapping_out_path` — plaintext CSV, or an AES-256-GCM-encrypted
/// JSON envelope if `mapping_passphrase` is given. `profile_out_path`, if
/// given, also mints a `DriftProfile`-shaped JSON file for `triplex
/// driftguard --baseline-profile` to consume later — optional and additive,
/// so every existing call site is unaffected by omitting it.
pub fn anonymize_file(
    input_path: &Path,
    recipe: &AnonymizeRecipe,
    output_path: &Path,
    mapping_out_path: &Path,
    mapping_passphrase: Option<&str>,
    excluded_row_indices: &HashSet<usize>,
    profile_out_path: Option<&Path>,
) -> Result<AnonymizeReport> {
    let sample = read_rows(input_path)?;
    let (sample, rows_excluded) = exclude_rows(sample, excluded_row_indices);
    let (resolved, drop_indices) = resolve_columns(recipe, &sample.headers, salt_from_env)?;
    let (masked, mapping_entries) = mask_rows(&resolved, sample);
    let columns_dropped: Vec<String> = drop_indices
        .iter()
        .map(|&i| masked.headers[i].clone())
        .collect();
    let risk_assessment = assess_bucket_risk(&resolved, &masked);
    let masked = drop_columns(masked, &drop_indices);

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

    if let Some(profile_path) = profile_out_path {
        mint_profile(output_path, risk_assessment, profile_path)?;
    }

    Ok(AnonymizeReport {
        rows_processed: masked.rows.len(),
        columns_anonymized: resolved.iter().map(|c| c.declared.name.clone()).collect(),
        columns_dropped,
        unique_masked_values_minted: mapping_entries.len(),
        risk_assessment,
        rows_excluded,
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
/// `excluded_row_indices` — see `exclude_rows`.
pub fn anonymize_bytes(
    bytes: &[u8],
    extension: &str,
    recipe: &AnonymizeRecipe,
    salts: &std::collections::HashMap<String, String>,
    excluded_row_indices: &HashSet<usize>,
) -> Result<AnonymizeBytesResult> {
    let sample = read_rows_bytes(bytes, extension)?;
    let (sample, rows_excluded) = exclude_rows(sample, excluded_row_indices);
    let (resolved, drop_indices) = resolve_columns(recipe, &sample.headers, salt_from_map(salts))?;
    let (masked, mapping_entries) = mask_rows(&resolved, sample);
    let columns_dropped: Vec<String> = drop_indices
        .iter()
        .map(|&i| masked.headers[i].clone())
        .collect();
    let risk_assessment = assess_bucket_risk(&resolved, &masked);
    let masked = drop_columns(masked, &drop_indices);

    let masked_csv = triplex_io::write_csv_bytes(&masked.headers, &masked.rows)?;
    let mapping_csv = mapping::mapping_to_csv(&mapping_entries)?;

    Ok(AnonymizeBytesResult {
        masked_csv,
        report: AnonymizeReport {
            rows_processed: masked.rows.len(),
            columns_anonymized: resolved.iter().map(|c| c.declared.name.clone()).collect(),
            columns_dropped,
            unique_masked_values_minted: mapping_entries.len(),
            risk_assessment,
            rows_excluded,
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

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new())
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

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new())
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

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new())
            .expect("anonymize_bytes should succeed");
        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        // ids 1..4 (recipe_with_method's default bucket_width: 10) all fall
        // in the same "0-9" bucket.
        assert!(masked.rows.iter().all(|r| r[0] == "0-9"));
        // All 4 rows share one equivalence class -> k-anonymity 4.
        assert_eq!(
            result.report.risk_assessment,
            Some(RiskAssessment::KAnonymity(4))
        );
    }

    #[test]
    fn anonymize_bytes_with_two_bucket_columns_scores_k_anonymity_on_the_composite() {
        let recipe = AnonymizeRecipe {
            version: 1,
            columns: vec![
                AnonymizeColumn {
                    name: "id".to_string(),
                    method: AnonymizeMethod::Bucket,
                    salt_env: "unused".to_string(),
                    bucket_width: 1, // no generalization: id stays its own class
                    keep_prefix: 0,
                    keep_suffix: 4,
                },
                AnonymizeColumn {
                    name: "price".to_string(),
                    method: AnonymizeMethod::Bucket,
                    salt_env: "unused".to_string(),
                    bucket_width: 10,
                    keep_prefix: 0,
                    keep_suffix: 4,
                },
            ],
        };
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        // Bucket ignores the salt *value*, but resolve_columns_with_salts
        // still requires an entry per declared non-Drop column.
        salts.insert("id".to_string(), "unused".to_string());
        salts.insert("price".to_string(), "unused".to_string());

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new())
            .expect("anonymize_bytes should succeed");

        // Composing id (width 1: every row its own bucket) with price still
        // leaves every row in its own equivalence class -> k-anonymity 1.
        assert_eq!(
            result.report.risk_assessment,
            Some(RiskAssessment::KAnonymity(1))
        );
    }

    #[test]
    fn anonymize_bytes_with_no_bucket_column_reports_no_risk_assessment() {
        let recipe = recipe_for("name", "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string());

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new())
            .expect("anonymize_bytes should succeed");

        assert_eq!(result.report.risk_assessment, None);
    }

    #[test]
    fn anonymize_bytes_excludes_the_given_row_indices_from_the_output() {
        let recipe = recipe_for("name", "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string());
        let excluded = HashSet::from([1]); // row 1: "2,3.50,false,gadget"

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &excluded)
            .expect("anonymize_bytes should succeed");
        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        assert_eq!(result.report.rows_excluded, 1);
        assert_eq!(result.report.rows_processed, 3);
        assert_eq!(masked.rows.len(), 3);
        assert!(masked.rows.iter().all(|r| r[0] != "2"));
    }

    #[test]
    fn anonymize_bytes_with_an_out_of_range_excluded_row_index_removes_nothing() {
        let recipe = recipe_for("name", "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string());
        let excluded = HashSet::from([99]);

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &excluded)
            .expect("anonymize_bytes should succeed");

        assert_eq!(result.report.rows_excluded, 0);
        assert_eq!(result.report.rows_processed, 4);
    }

    #[test]
    fn anonymize_bytes_with_excluded_rows_scores_risk_on_the_remaining_rows_only() {
        // id bucketed at width 1 (no generalization -- every id is its own
        // class): without exclusion this is k-anonymity 1 regardless: with
        // all 4 distinct ids, excluding one still leaves the rest at 1.
        // What this actually guards is unique_masked_values_minted/
        // rows_processed reflecting 3 rows, not 4 -- an excluded row's id
        // must never reach assess_bucket_risk (or mask_rows) at all.
        let mut recipe = recipe_with_method("id", AnonymizeMethod::Bucket, "unused");
        recipe.columns[0].bucket_width = 1;
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("id".to_string(), "unused".to_string());
        let excluded = HashSet::from([1]); // id "2"

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &excluded)
            .expect("anonymize_bytes should succeed");
        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        assert_eq!(result.report.rows_processed, 3);
        assert!(masked.rows.iter().all(|r| r[0] != "2"));
        assert_eq!(
            result.report.risk_assessment,
            Some(RiskAssessment::KAnonymity(1))
        );
    }

    #[test]
    fn anonymize_bytes_with_all_rows_excluded_reports_zero_rows_processed() {
        let recipe = recipe_for("name", "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string());
        let excluded = HashSet::from([0, 1, 2, 3]);

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &excluded)
            .expect("anonymize_bytes should succeed even with an empty result");

        assert_eq!(result.report.rows_excluded, 4);
        assert_eq!(result.report.rows_processed, 0);
        assert_eq!(result.report.unique_masked_values_minted, 0);
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

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new())
            .expect("anonymize_bytes should succeed");
        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        // recipe_with_method's default keep_suffix: 4 — "widget" keeps "dget".
        assert_eq!(masked.rows[0][3], "**dget");
    }

    #[test]
    fn anonymize_bytes_with_drop_removes_the_column_entirely_and_needs_no_salt() {
        let recipe = recipe_with_method("name", AnonymizeMethod::Drop, "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let salts = std::collections::HashMap::new(); // empty -- Drop needs no salt

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new())
            .expect("anonymize_bytes should succeed with no salt for a Drop-only recipe");
        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        assert_eq!(masked.headers, vec!["id", "price", "in_stock"]);
        assert_eq!(masked.rows[0], vec!["1", "9.99", "true"]);
        assert_eq!(result.report.columns_dropped, vec!["name"]);
        assert!(result.report.columns_anonymized.is_empty());
        // Nothing to reverse for a dropped column -- no mapping entry at all.
        let mapping = triplex_io::read_csv_full_bytes(&result.mapping_csv).unwrap();
        assert!(mapping.rows.is_empty());
    }

    #[test]
    fn anonymize_bytes_with_drop_and_a_masked_column_keeps_indices_aligned() {
        let recipe = AnonymizeRecipe {
            version: 1,
            columns: vec![
                AnonymizeColumn {
                    name: "id".to_string(),
                    method: AnonymizeMethod::Drop,
                    salt_env: "unused".to_string(),
                    bucket_width: 10,
                    keep_prefix: 0,
                    keep_suffix: 4,
                },
                AnonymizeColumn {
                    name: "name".to_string(),
                    method: AnonymizeMethod::TokenHmac,
                    salt_env: "unused".to_string(),
                    bucket_width: 10,
                    keep_prefix: 0,
                    keep_suffix: 4,
                },
            ],
        };
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string()); // no "id" entry needed

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new())
            .expect("anonymize_bytes should succeed");
        let masked = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        // "id" (originally index 0) is gone; "price"/"in_stock" keep their
        // original values at their shifted-left positions; "name" (now
        // index 2) is masked, not just relocated.
        assert_eq!(masked.headers, vec!["price", "in_stock", "name"]);
        assert_eq!(masked.rows[0][0], "9.99");
        assert_eq!(masked.rows[0][1], "true");
        assert_ne!(masked.rows[0][2], "widget");
        assert_eq!(masked.rows[0][2].len(), 64); // hex-encoded HMAC-SHA256
    }

    #[test]
    fn anonymize_bytes_matches_anonymize_file_for_the_same_input_and_salt() {
        std::env::set_var("TRIPLEX_TEST_SALT_BYTES_PARITY", "test-salt");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_BYTES_PARITY");
        let output = temp_path("output_bytes_parity.csv");
        let mapping_out = temp_path("mapping_bytes_parity.csv");

        anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            None,
            &HashSet::new(),
            None,
        )
        .expect("anonymize_file should succeed");
        let via_path = triplex_io::read_csv_full(&output).unwrap();
        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();

        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let mut salts = std::collections::HashMap::new();
        salts.insert("name".to_string(), "test-salt".to_string());
        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new()).unwrap();
        let via_bytes = triplex_io::read_csv_full_bytes(&result.masked_csv).unwrap();

        assert_eq!(
            via_bytes.rows, via_path.rows,
            "the same salt must mint the same tokens regardless of delivery surface"
        );
    }

    #[test]
    fn anonymize_bytes_with_no_salt_for_a_declared_column_is_a_clear_error_not_a_panic() {
        let recipe = recipe_for("name", "unused_in_the_bytes_path");
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let salts = std::collections::HashMap::new(); // no "name" entry

        let result = anonymize_bytes(&bytes, "csv", &recipe, &salts, &HashSet::new());
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

        let result = anonymize_bytes(&bytes, "parquet", &recipe, &salts, &HashSet::new());
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
            &HashSet::new(),
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
    fn anonymize_file_excludes_the_given_row_indices_from_the_output() {
        std::env::set_var("TRIPLEX_TEST_SALT_EXCLUDE_ROWS", "test-salt");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_EXCLUDE_ROWS");
        let output = temp_path("output_exclude_rows.csv");
        let mapping_out = temp_path("mapping_exclude_rows.csv");
        let excluded = HashSet::from([1, 3]); // "gadget" and "doohickey"

        let report = anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            None,
            &excluded,
            None,
        )
        .expect("anonymize should succeed");

        assert_eq!(report.rows_excluded, 2);
        assert_eq!(report.rows_processed, 2);
        let masked = triplex_io::read_csv_full(&output).unwrap();
        assert_eq!(masked.rows.len(), 2);
        assert_eq!(masked.rows[0][0], "1");
        assert_eq!(masked.rows[1][0], "3");

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
            &HashSet::new(),
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

        anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            None,
            &HashSet::new(),
            None,
        )
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

        let result = anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            None,
            &HashSet::new(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn a_missing_salt_env_var_is_a_clear_error_not_a_panic() {
        std::env::remove_var("TRIPLEX_TEST_SALT_DEFINITELY_UNSET");
        let recipe = recipe_for("name", "TRIPLEX_TEST_SALT_DEFINITELY_UNSET");
        let output = temp_path("output_5.csv");
        let mapping_out = temp_path("mapping_5.csv");

        let result = anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            None,
            &HashSet::new(),
            None,
        );
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
            &HashSet::new(),
            None,
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
            &HashSet::new(),
            None,
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
            &HashSet::new(),
            None,
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

        anonymize_file(
            &fixture("sample.csv"),
            &recipe,
            &output,
            &mapping_out,
            None,
            &HashSet::new(),
            None,
        )
        .expect("anonymize should succeed");

        let result = reveal_mapping(&mapping_out, "any passphrase", &revealed);
        assert!(result.is_err());

        std::fs::remove_file(&output).ok();
        std::fs::remove_file(&mapping_out).ok();
    }
}
