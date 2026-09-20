//! Structural firewall: infers a schema contract from a local data sample and
//! emits it as versioned, deterministic JSON. No network calls; the caller's
//! path is the only source of data, and nothing here transmits it anywhere.

mod contract;
mod convert;
mod semantic;
pub use contract::{
    check_patterns, check_patterns_bytes, check_patterns_path, load_contract, parse_contract,
    to_declared_contract, validate, validate_csv_with_delimiter, validate_path,
    verify_contract_well_formed, DeclaredColumn, DeclaredContract, DeclaredDriftGuardThresholds,
    DeclaredFairnessThresholds, DeclaredModelCard, DeclaredModelMetadata, DeclaredSynthesize,
    DeclaredSynthesizeColumn, DeclaredSynthesizeKind, ValidationError, ValidationReport,
};
pub use convert::{
    convert_bytes, convert_file, convert_file_with_delimiters, read_full_bytes, ConvertReport,
};
pub use semantic::SemanticType;

use serde::{Deserialize, Serialize};
use std::path::Path;
use triplex_core::{normalize_numeric_string, Error, Result};
use triplex_io::Sample;

const CONTRACT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    Integer,
    Float,
    Boolean,
    String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub inferred_type: ColumnType,
    /// Sensitive-column hint layered on top of `inferred_type` — e.g. a
    /// `String` column whose values checksum as IBANs, or whose header says
    /// "address". `None` when nothing matched; see `semantic.rs`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_type: Option<SemanticType>,
    pub type_consistency_percentage: f64,
    /// Null rate and numeric range/mean, computed once by triplex-io and
    /// shared by every ingestion format — flattened so the JSON shape is
    /// unchanged (null_percentage/min_value/max_value/mean_value stay
    /// top-level keys, not nested under "statistics").
    #[serde(flatten)]
    pub statistics: triplex_io::ColumnStatistics,
    /// Quantiles/histogram (numeric) or top-value counts (categorical) for
    /// the browser table's header sparklines and column-detail view. `None`
    /// for Parquet — footer-only stats carry no raw values to bucket or
    /// rank, the same reason its `mean_value` is also `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution: Option<triplex_io::ColumnDistribution>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SchemaContract {
    pub version: String,
    pub columns: Vec<ColumnSchema>,
}

/// `f64::from_str` accepts "nan"/"inf"/"infinity" (case-insensitively) as valid
/// floats, but those are missing-value placeholders in practice, not numbers —
/// excluding them keeps a `["1", "2", "nan"]` column from inferring as Float.
/// `v` should already be normalized via `normalize_numeric_string`.
fn parses_as_finite_float(v: &str) -> bool {
    v.parse::<f64>().is_ok_and(f64::is_finite)
}

/// A whole-number-looking string with a padding zero — `"007"`, `"-0042"` —
/// is a zip code, an employee ID, a product code: an identifier, not a
/// quantity. `i64`/`f64` parsing accepts it anyway (`"007"` parses to `7`),
/// so without this check it would infer as Integer/Float and silently lose
/// its padding the moment it's written back out through a real numeric type
/// (e.g. `triplex convert`'s Parquet output: `"007"` → `7` → `"7"`).
/// `"0"` alone, and a genuine decimal like `"0.5"` (the zero is immediately
/// followed by `.`, not more digits), are unaffected. `v` should already be
/// normalized via `normalize_numeric_string`.
fn has_meaningless_leading_zero(v: &str) -> bool {
    let digits = v.strip_prefix('-').unwrap_or(v);
    !digits.contains('.') && digits.len() > 1 && digits.starts_with('0')
}

/// A value is null for schema purposes when the CSV cell is empty. Distinguishing
/// "empty string" from "absent" would need a quoting-aware signal CSV doesn't give us.
fn infer_column_type(non_empty: &[&&str]) -> ColumnType {
    if non_empty.is_empty() {
        return ColumnType::String;
    }
    if non_empty.iter().all(|v| {
        let n = normalize_numeric_string(v);
        n.parse::<i64>().is_ok() && !has_meaningless_leading_zero(&n)
    }) {
        return ColumnType::Integer;
    }
    if non_empty.iter().all(|v| {
        let n = normalize_numeric_string(v);
        parses_as_finite_float(&n) && !has_meaningless_leading_zero(&n)
    }) {
        return ColumnType::Float;
    }
    if non_empty
        .iter()
        .all(|v| v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("false"))
    {
        return ColumnType::Boolean;
    }
    ColumnType::String
}

/// When a column doesn't cleanly infer to Integer/Float/Boolean, this is how much
/// of it actually would have: the best match rate among those types, e.g. 9 clean
/// integers and 1 stray "n/a" is 90% — an accidental type mix, not a real string
/// column. A genuine string column (no numeric/boolean look-alikes at all) is 100%.
fn type_consistency(non_empty: &[&&str], inferred_type: ColumnType) -> f64 {
    if non_empty.is_empty() || inferred_type != ColumnType::String {
        return 100.0;
    }
    let integer_matches = non_empty
        .iter()
        .filter(|v| normalize_numeric_string(v).parse::<i64>().is_ok())
        .count();
    let float_matches = non_empty
        .iter()
        .filter(|v| parses_as_finite_float(&normalize_numeric_string(v)))
        .count();
    let bool_matches = non_empty
        .iter()
        .filter(|v| v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("false"))
        .count();
    let best = integer_matches.max(float_matches).max(bool_matches);
    if best == 0 {
        100.0
    } else {
        best as f64 / non_empty.len() as f64 * 100.0
    }
}

/// Infers a schema contract from an already-loaded sample.
pub fn infer_schema(sample: &Sample) -> SchemaContract {
    let columns = sample
        .headers
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let values: Vec<&str> = sample
                .rows
                .iter()
                .map(|row| row.get(i).map(String::as_str).unwrap_or(""))
                .collect();
            let non_empty: Vec<&&str> = values.iter().filter(|v| !v.is_empty()).collect();
            let inferred_type = infer_column_type(&non_empty);
            let value_kind = match inferred_type {
                ColumnType::Integer | ColumnType::Float => triplex_io::ValueKind::Numeric,
                ColumnType::Boolean | ColumnType::String => triplex_io::ValueKind::Categorical,
            };
            let non_empty_values: Vec<&str> = non_empty.iter().map(|v| **v).collect();
            let semantic_type = match inferred_type {
                ColumnType::String => semantic::detect_semantic_type(name, &non_empty_values),
                ColumnType::Integer => semantic::detect_integer_semantic_type(&non_empty_values),
                ColumnType::Float | ColumnType::Boolean => None,
            };
            ColumnSchema {
                name: name.clone(),
                inferred_type,
                semantic_type,
                type_consistency_percentage: type_consistency(&non_empty, inferred_type),
                statistics: triplex_io::compute_column_statistics(&values),
                distribution: Some(triplex_io::compute_column_distribution(&values, value_kind)),
            }
        })
        .collect();

    SchemaContract {
        version: CONTRACT_VERSION.to_string(),
        columns,
    }
}

/// Maps a Parquet physical type to our primitive `ColumnType`. Parquet's
/// INT96 (legacy timestamp) and logical DATE/TIMESTAMP annotations fall back
/// to String — no temporal type yet, matching the CSV inference path.
fn column_type_from_parquet_physical_type(physical_type: &str) -> ColumnType {
    match physical_type {
        "BOOLEAN" => ColumnType::Boolean,
        "INT32" | "INT64" => ColumnType::Integer,
        "FLOAT" | "DOUBLE" => ColumnType::Float,
        _ => ColumnType::String,
    }
}

/// Infers a schema contract from a local Parquet file's footer metadata only
/// — the row data itself is never decoded, so this is cheap regardless of
/// file size.
fn infer_schema_from_parquet(path: &Path) -> Result<SchemaContract> {
    let stats = triplex_io::read_parquet_schema(path)?;
    let columns = stats
        .into_iter()
        .map(|s| {
            let inferred_type = column_type_from_parquet_physical_type(&s.physical_type);
            // No raw values in Parquet footer stats, only header-keyword
            // detection (Address/Location) can fire here — see `semantic.rs`.
            let semantic_type = (inferred_type == ColumnType::String)
                .then(|| semantic::detect_semantic_type(&s.name, &[]))
                .flatten();
            ColumnSchema {
                name: s.name,
                inferred_type,
                semantic_type,
                // Parquet is columnar with one physical type per column — no
                // per-value ambiguity, so there's nothing to be inconsistent with.
                type_consistency_percentage: 100.0,
                statistics: s.statistics,
                distribution: None,
            }
        })
        .collect();

    Ok(SchemaContract {
        version: CONTRACT_VERSION.to_string(),
        columns,
    })
}

pub(crate) fn unsupported_extension_error(path: &Path) -> Error {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    Error::UnsupportedExtension(format!(
        "unsupported file extension \"{ext}\" (expected .csv, .tsv, .jsonl, or .parquet)"
    ))
}

/// Reads a local data file and infers its schema contract in one step.
/// Dispatches on the file extension: `.parquet` reads the file's own
/// embedded schema, so `sample_rows` is unused for it; every other known
/// extension (`.csv`/`.tsv`/`.jsonl`) picks its `SampleFormat` strategy
/// (Strategy pattern — see `triplex_io::format`) and samples up to
/// `sample_rows` rows from it.
pub fn infer_schema_from_path(path: &Path, sample_rows: usize) -> Result<SchemaContract> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    if extension == "parquet" {
        return infer_schema_from_parquet(path);
    }

    let format = triplex_io::sample_format_for_extension(extension)
        .ok_or_else(|| unsupported_extension_error(path))?;
    let sample = format.read_sample(path, sample_rows)?;
    Ok(infer_schema(&sample))
}

/// Infers a schema contract from a `.csv`-shaped file whose actual field
/// separator isn't a comma — e.g. the semicolon-delimited exports common
/// under European Excel locale settings, or a pipe-delimited feed.
/// Deliberately bypasses the extension-based `SampleFormat` dispatch: a
/// custom delimiter is an explicit override, not something to infer from
/// the file extension.
pub fn infer_schema_from_csv_with_delimiter(
    path: &Path,
    delimiter: u8,
    sample_rows: usize,
) -> Result<SchemaContract> {
    let sample = triplex_io::read_csv_sample_with_delimiter(path, delimiter, sample_rows)?;
    Ok(infer_schema(&sample))
}

/// Infers a schema contract from an in-memory byte slice instead of a local
/// path — for a caller with no filesystem (`apps/triplex-web`'s
/// browser/WASM upload flow). Reads the whole buffer rather than sampling:
/// unlike a CLI run against an arbitrarily large local file, a browser
/// upload is already bounded by what a user selected and is already fully
/// in memory, so there's no separate cost to bound. `.csv`/`.tsv`/`.jsonl`
/// only — see `convert::read_full_bytes` for why `.parquet` isn't supported
/// here.
pub fn infer_schema_from_bytes(bytes: &[u8], extension: &str) -> Result<SchemaContract> {
    let sample = convert::read_full_bytes(bytes, extension)?;
    Ok(infer_schema(&sample))
}

/// The browser table view's single entry point: parses `bytes` **once**
/// (unlike calling `infer_schema_from_bytes` and a separate full-row read
/// side by side, which each independently re-parse the whole file) and
/// returns both the inferred schema and the row data serialized as Arrow
/// IPC bytes — typed columns, not a JSON string of every stringified cell.
/// See the design notes.
pub fn read_and_profile_bytes(bytes: &[u8], extension: &str) -> Result<(SchemaContract, Vec<u8>)> {
    let sample = convert::read_full_bytes(bytes, extension)?;
    let schema = infer_schema(&sample);
    let column_types: Vec<triplex_io::WriteColumnType> = schema
        .columns
        .iter()
        .map(|c| convert::to_write_column_type(c.inferred_type))
        .collect();
    let table_arrow = triplex_io::rows_to_arrow_ipc(&sample.headers, &column_types, &sample.rows)?;
    Ok((schema, table_arrow))
}

/// Infers a schema from bytes and renders it as a starting `.triplex.yaml`
/// contract — `apps/triplex-web`'s "generate contract" action in one call.
/// See `to_declared_contract` for the inferred-to-declared mapping.
pub fn generate_contract_yaml_from_bytes(bytes: &[u8], extension: &str) -> Result<String> {
    let inferred = infer_schema_from_bytes(bytes, extension)?;
    let declared = to_declared_contract(&inferred);
    serde_yaml_ng::to_string(&declared).map_err(|e| Error::Yaml(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{Duration, Instant};

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name)
    }

    #[test]
    fn three_decimal_probabilities_infer_as_float_with_the_right_statistics() {
        // 0.639 used to normalize to 639, then fail the leading-zero check and infer as String.
        let sample = Sample {
            headers: vec!["score".into()],
            rows: ["0.639", "0.5", "0.641", "0.001"]
                .iter()
                .map(|v| vec![v.to_string()])
                .collect(),
        };
        let column = &infer_schema(&sample).columns[0];
        assert_eq!(column.inferred_type, ColumnType::Float);
        assert_eq!(column.statistics.max_value, Some(0.641));
        assert_eq!(column.statistics.min_value, Some(0.001));
        let mean = column.statistics.mean_value.unwrap();
        assert!((mean - 0.44525).abs() < 1e-12, "mean {mean}");
    }

    #[test]
    fn infers_each_primitive_type() {
        let sample = Sample {
            headers: vec![
                "id".into(),
                "price".into(),
                "in_stock".into(),
                "name".into(),
            ],
            rows: vec![
                vec!["1".into(), "9.99".into(), "true".into(), "widget".into()],
                vec!["2".into(), "3.50".into(), "false".into(), "gadget".into()],
            ],
        };

        let contract = infer_schema(&sample);

        assert_eq!(contract.version, CONTRACT_VERSION);
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[2].inferred_type, ColumnType::Boolean);
        assert_eq!(contract.columns[3].inferred_type, ColumnType::String);

        assert!(
            matches!(
                contract.columns[0].distribution,
                Some(triplex_io::ColumnDistribution::Numeric { .. })
            ),
            "an Integer column should get a Numeric distribution"
        );
        assert!(
            matches!(
                contract.columns[3].distribution,
                Some(triplex_io::ColumnDistribution::Categorical { .. })
            ),
            "a String column should get a Categorical distribution"
        );
    }

    #[test]
    fn a_column_of_zero_padded_codes_infers_as_string_not_integer() {
        let sample = Sample {
            headers: vec!["zip".into()],
            rows: vec![vec!["007".into()], vec!["012".into()], vec!["099".into()]],
        };
        let contract = infer_schema(&sample);
        assert_eq!(
            contract.columns[0].inferred_type,
            ColumnType::String,
            "a leading zero means this is an identifier, not a quantity — \
             typing it as Integer would silently corrupt it to \"7\"/\"12\"/\"99\""
        );
    }

    #[test]
    fn a_bare_zero_and_a_genuine_decimal_are_unaffected_by_the_leading_zero_rule() {
        let sample = Sample {
            headers: vec!["count".into(), "ratio".into()],
            rows: vec![vec!["0".into(), "0.5".into()]],
        };
        let contract = infer_schema(&sample);
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
    }

    #[test]
    fn trailing_sign_amounts_infer_as_numeric_with_correct_range() {
        let sample = Sample {
            headers: vec!["amount".into(), "qty".into()],
            rows: vec![
                vec!["150.00-".into(), "3-".into()],
                vec!["42.50+".into(), "7".into()],
            ],
        };
        let contract = infer_schema(&sample);
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[0].statistics.min_value, Some(-150.0));
        assert_eq!(contract.columns[0].statistics.max_value, Some(42.50));
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].statistics.min_value, Some(-3.0));
        assert_eq!(contract.columns[1].statistics.max_value, Some(7.0));
    }

    #[test]
    fn currency_thousands_and_parenthesized_amounts_infer_as_numeric() {
        let sample = Sample {
            headers: vec!["amount".into(), "price".into()],
            rows: vec![
                vec!["$1,234.56".into(), "150,00".into()],
                vec!["(150.00)".into(), "42,50".into()],
            ],
        };
        let contract = infer_schema(&sample);
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[0].statistics.min_value, Some(-150.0));
        assert_eq!(contract.columns[0].statistics.max_value, Some(1234.56));
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[1].statistics.min_value, Some(42.50));
        assert_eq!(contract.columns[1].statistics.max_value, Some(150.0));
    }

    /// Not run by default (`cargo test`). Regenerates `tests/fixtures/sample.parquet`
    /// deterministically via `cargo test -p triplex-schemacast -- --ignored
    /// generates_the_parquet_fixture`. Kept as a test rather than a script so the
    /// fixture's exact shape lives next to the tests that assert on it.
    #[test]
    #[ignore]
    fn generates_the_parquet_fixture() {
        use parquet::data_type::{BoolType, ByteArray, ByteArrayType, DoubleType, Int64Type};
        use parquet::file::properties::WriterProperties;
        use parquet::file::writer::SerializedFileWriter;
        use parquet::schema::parser::parse_message_type;
        use std::sync::Arc;

        let message_type = "
            message schema {
                REQUIRED INT64 id;
                OPTIONAL DOUBLE price;
                REQUIRED BOOLEAN in_stock;
                REQUIRED BYTE_ARRAY name (UTF8);
            }
        ";
        let schema = Arc::new(parse_message_type(message_type).unwrap());
        let props = Arc::new(WriterProperties::builder().build());
        let file = std::fs::File::create(fixture("sample.parquet")).unwrap();
        let mut writer = SerializedFileWriter::new(file, schema, props).unwrap();
        let mut row_group_writer = writer.next_row_group().unwrap();

        let mut col = row_group_writer.next_column().unwrap().unwrap();
        col.typed::<Int64Type>()
            .write_batch(&[1, 2, 3, 4], None, None)
            .unwrap();
        col.close().unwrap();

        // One null price (row 4), to exercise null-count statistics too.
        let mut col = row_group_writer.next_column().unwrap().unwrap();
        col.typed::<DoubleType>()
            .write_batch(&[9.99, 3.50, 12.00], Some(&[1, 1, 1, 0]), None)
            .unwrap();
        col.close().unwrap();

        let mut col = row_group_writer.next_column().unwrap().unwrap();
        col.typed::<BoolType>()
            .write_batch(&[true, false, true, true], None, None)
            .unwrap();
        col.close().unwrap();

        let mut col = row_group_writer.next_column().unwrap().unwrap();
        let names = ["widget", "gadget", "sprocket", "doohickey"]
            .map(|s| ByteArray::from(s.as_bytes().to_vec()));
        col.typed::<ByteArrayType>()
            .write_batch(&names, None, None)
            .unwrap();
        col.close().unwrap();

        row_group_writer.close().unwrap();
        writer.close().unwrap();
    }

    #[test]
    fn reads_and_infers_from_a_parquet_files_own_embedded_schema() {
        let contract = infer_schema_from_path(&fixture("sample.parquet"), 100)
            .expect("parquet fixture should be readable");

        assert_eq!(
            contract
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "price", "in_stock", "name"]
        );
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[2].inferred_type, ColumnType::Boolean);
        assert_eq!(contract.columns[3].inferred_type, ColumnType::String);
        assert!(
            contract.columns.iter().all(|c| c.distribution.is_none()),
            "Parquet's footer-only stats carry no raw values to bucket or rank"
        );
    }

    #[test]
    fn parquet_null_counts_come_from_footer_statistics_not_a_full_scan() {
        let stats = triplex_io::read_parquet_schema(&fixture("sample.parquet"))
            .expect("parquet fixture should be readable");
        let price = stats.iter().find(|s| s.name == "price").unwrap();
        assert_eq!(price.statistics.null_percentage, 25.0); // 1 null of 4 rows
    }

    #[test]
    fn parquet_min_max_come_from_footer_statistics_too() {
        let contract = infer_schema_from_path(&fixture("sample.parquet"), 100)
            .expect("parquet fixture should be readable");
        let price = &contract.columns[1];
        assert_eq!(price.statistics.min_value, Some(3.50));
        assert_eq!(price.statistics.max_value, Some(12.00));

        let name = &contract.columns[3];
        assert_eq!(
            name.statistics.min_value, None,
            "a string column has no numeric range"
        );
    }

    #[test]
    fn an_unsupported_extension_is_a_clear_error_not_a_panic() {
        let result = infer_schema_from_path(Path::new("data.xlsx"), 100);
        assert!(result.is_err());
    }

    #[test]
    fn infers_a_schema_from_tsv_the_same_way_as_csv() {
        let contract = infer_schema_from_path(&fixture("sample.tsv"), 100)
            .expect("tsv fixture should be readable");

        assert_eq!(
            contract
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "price", "in_stock", "name"]
        );
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[2].inferred_type, ColumnType::Boolean);
        assert_eq!(contract.columns[3].inferred_type, ColumnType::String);
    }

    #[test]
    fn infers_a_schema_from_a_semicolon_delimited_csv_with_an_explicit_delimiter() {
        let contract =
            infer_schema_from_csv_with_delimiter(&fixture("sample.semicolon.csv"), b';', 100)
                .expect("semicolon fixture should be readable");

        assert_eq!(
            contract
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "price", "in_stock", "name"]
        );
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
    }

    #[test]
    fn infers_a_schema_from_jsonl() {
        let contract = infer_schema_from_path(&fixture("sample.jsonl"), 100)
            .expect("jsonl fixture should be readable");

        assert_eq!(
            contract
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "price", "in_stock", "name"]
        );
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[2].inferred_type, ColumnType::Boolean);
        assert_eq!(contract.columns[3].inferred_type, ColumnType::String);
        // Row 4's price is JSON null, same treatment as an empty CSV cell.
        assert_eq!(contract.columns[1].statistics.null_percentage, 25.0);
    }

    #[test]
    fn infer_schema_from_bytes_matches_the_path_based_reader() {
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let from_bytes = infer_schema_from_bytes(&bytes, "csv").expect("bytes should infer");
        let from_path = infer_schema_from_path(&fixture("sample.csv"), 100).unwrap();
        assert_eq!(
            from_bytes
                .columns
                .iter()
                .map(|c| c.inferred_type)
                .collect::<Vec<_>>(),
            from_path
                .columns
                .iter()
                .map(|c| c.inferred_type)
                .collect::<Vec<_>>()
        );
    }

    /// The single-parse path must agree with the old two-call path it
    /// replaces: same inferred schema, and the Arrow IPC bytes decode back
    /// to the exact same headers/values `infer_schema_from_bytes` saw.
    #[test]
    fn read_and_profile_bytes_matches_infer_schema_from_bytes_and_round_trips_through_arrow() {
        use arrow_array::Array;
        use arrow_ipc::reader::StreamReader;
        use std::io::Cursor;

        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let (schema, table_arrow) =
            read_and_profile_bytes(&bytes, "csv").expect("bytes should parse and profile");
        let expected_schema = infer_schema_from_bytes(&bytes, "csv").unwrap();

        assert_eq!(
            schema
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            expected_schema
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            schema
                .columns
                .iter()
                .map(|c| c.inferred_type)
                .collect::<Vec<_>>(),
            expected_schema
                .columns
                .iter()
                .map(|c| c.inferred_type)
                .collect::<Vec<_>>(),
        );

        let mut reader = StreamReader::try_new(Cursor::new(table_arrow), None).unwrap();
        let field_names: Vec<String> = reader
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect();
        assert_eq!(field_names, vec!["id", "price", "in_stock", "name"]);

        let batch = reader.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 4);
        let ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<arrow_array::Int64Array>()
            .expect("id is Integer-inferred, so it must come back as a typed Int64Array");
        assert_eq!(ids.value(0), 1);
    }

    #[test]
    fn generate_contract_yaml_from_bytes_round_trips_through_parse_contract() {
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let yaml =
            generate_contract_yaml_from_bytes(&bytes, "csv").expect("generation should succeed");

        let declared = parse_contract(&yaml).expect("generated yaml should itself be parseable");
        assert_eq!(
            declared
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "price", "in_stock", "name"]
        );
        assert_eq!(declared.columns[0].column_type, ColumnType::Integer);
        assert!(declared.columns.iter().all(|c| c.required));

        // The generated contract must itself validate cleanly against the
        // same data it was inferred from.
        let inferred = infer_schema_from_bytes(&bytes, "csv").unwrap();
        assert!(validate(&declared, &inferred).valid);
    }

    #[test]
    fn reads_and_infers_from_the_synthetic_fixture() {
        let contract = infer_schema_from_path(&fixture("sample.csv"), 100)
            .expect("fixture should be readable");

        assert_eq!(
            contract
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "price", "in_stock", "name"]
        );
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[2].inferred_type, ColumnType::Boolean);
        assert_eq!(contract.columns[3].inferred_type, ColumnType::String);
    }

    #[test]
    fn handles_quoted_commas_escaped_quotes_unicode_and_scientific_notation() {
        let contract = infer_schema_from_path(&fixture("sample.tricky.csv"), 100)
            .expect("tricky fixture should be readable despite quoting/unicode");

        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer); // id
        assert_eq!(contract.columns[1].inferred_type, ColumnType::String); // description
        assert_eq!(contract.columns[2].inferred_type, ColumnType::Integer); // amount, incl. negatives
        assert_eq!(contract.columns[3].inferred_type, ColumnType::Float); // ratio, scientific notation

        // Row 4's blanks make description/amount/ratio 25% null; id has none.
        assert_eq!(contract.columns[0].statistics.null_percentage, 0.0);
        assert_eq!(contract.columns[1].statistics.null_percentage, 25.0);
    }

    #[test]
    fn csv_observed_range_matches_the_sampled_values() {
        let contract = infer_schema_from_path(&fixture("sample.csv"), 100)
            .expect("fixture should be readable");
        let price = &contract.columns[1];
        assert_eq!(price.statistics.min_value, Some(3.50));
        assert_eq!(price.statistics.max_value, Some(12.00));
        assert_eq!(
            price.statistics.mean_value,
            Some((9.99 + 3.50 + 12.00) / 3.0)
        );

        let name = &contract.columns[3];
        assert_eq!(
            name.statistics.min_value, None,
            "a string column has no numeric range"
        );
        assert_eq!(name.statistics.mean_value, None);
    }

    #[test]
    fn parquet_has_no_mean_since_footer_statistics_carry_no_sum() {
        let contract = infer_schema_from_path(&fixture("sample.parquet"), 100)
            .expect("parquet fixture should be readable");
        let price = &contract.columns[1];
        assert_eq!(
            price.statistics.mean_value, None,
            "computing a real mean would require decoding row data, breaking zero-copy"
        );
    }

    #[test]
    fn an_entirely_empty_column_is_100_percent_null_and_defaults_to_string() {
        let contract = infer_schema_from_path(&fixture("sample.all_null_column.csv"), 100)
            .expect("fixture should be readable");

        let note = &contract.columns[1];
        assert_eq!(note.name, "note");
        assert_eq!(note.inferred_type, ColumnType::String);
        assert_eq!(note.statistics.null_percentage, 100.0);
    }

    #[test]
    fn nan_and_inf_placeholders_do_not_infer_a_column_as_float() {
        let sample = Sample {
            headers: vec!["reading".into()],
            rows: vec![
                vec!["1".into()],
                vec!["2".into()],
                vec!["nan".into()],
                vec!["4".into()],
            ],
        };

        let contract = infer_schema(&sample);

        assert_eq!(contract.columns[0].inferred_type, ColumnType::String);
        assert_eq!(contract.columns[0].type_consistency_percentage, 75.0);
    }

    #[test]
    fn a_column_with_a_few_stray_values_infers_as_string_but_reports_the_mix() {
        let contract = infer_schema_from_path(&fixture("sample.mixed_types.csv"), 100)
            .expect("fixture should be readable");

        // 3 of 4 "quantity" values are integers; one ("not_a_number") is not, so the
        // waterfall falls back to String — but type_consistency_percentage should
        // still surface that this was an accidental mix, not a genuine string column.
        let quantity = &contract.columns[1];
        assert_eq!(quantity.name, "quantity");
        assert_eq!(quantity.inferred_type, ColumnType::String);
        assert_eq!(quantity.type_consistency_percentage, 75.0);

        // A genuinely string column (no numeric/boolean look-alikes) stays at 100%.
        let name = &contract.columns[2];
        assert_eq!(name.name, "name");
        assert_eq!(name.inferred_type, ColumnType::String);
        assert_eq!(name.type_consistency_percentage, 100.0);

        // A clean numeric column is also 100% consistent.
        assert_eq!(contract.columns[0].type_consistency_percentage, 100.0);
    }

    #[test]
    fn ragged_rows_are_a_reported_error_not_a_panic_or_silent_misread() {
        let result = infer_schema_from_path(&fixture("sample.ragged.csv"), 100);
        assert!(
            result.is_err(),
            "rows with the wrong field count must fail loudly, not be silently misaligned"
        );
    }

    /// Bounded-memory / bounded-latency check: sampling must stay cheap regardless
    /// of total file size, since only `sample_rows` records are ever read.
    #[test]
    fn inference_stays_fast_on_a_file_with_a_hundred_thousand_rows() {
        const ROW_COUNT: usize = 100_000;
        const SAMPLE_ROWS: usize = 1000;

        let path = std::env::temp_dir().join(format!(
            "triplex_schemacast_large_fixture_{}.csv",
            std::process::id()
        ));
        {
            let file = std::fs::File::create(&path).expect("temp file should be creatable");
            let mut writer = std::io::BufWriter::new(file);
            writeln!(writer, "id,score,label").unwrap();
            for i in 0..ROW_COUNT {
                writeln!(writer, "{i},{i}.5,row{i}").unwrap();
            }
        }

        let started = Instant::now();
        let contract = infer_schema_from_path(&path, SAMPLE_ROWS);
        let elapsed = started.elapsed();
        std::fs::remove_file(&path).ok();

        let contract = contract.expect("large synthetic file should be readable");
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[2].inferred_type, ColumnType::String);
        assert!(
            elapsed < Duration::from_secs(2),
            "inference took {elapsed:?} for a {ROW_COUNT}-row file with a {SAMPLE_ROWS}-row \
             sample — sampling should make this independent of total file size"
        );
    }
}
