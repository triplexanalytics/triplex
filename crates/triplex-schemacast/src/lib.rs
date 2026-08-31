//! Structural firewall: infers a schema contract from a local data sample and
//! emits it as versioned, deterministic JSON. No network calls; the caller's
//! path is the only source of data, and nothing here transmits it anywhere.

mod contract;
pub use contract::{
    validate, validate_path, DeclaredColumn, DeclaredContract, ValidationError, ValidationReport,
};

use serde::{Deserialize, Serialize};
use std::path::Path;
use triplex_core::{Error, Result};
use triplex_io::CsvSample;

const CONTRACT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    Integer,
    Float,
    Boolean,
    String,
}

#[derive(Debug, Serialize)]
pub struct ColumnSchema {
    pub name: String,
    pub inferred_type: ColumnType,
}

#[derive(Debug, Serialize)]
pub struct SchemaContract {
    pub version: String,
    pub columns: Vec<ColumnSchema>,
}

fn infer_column_type(values: &[&str]) -> ColumnType {
    let non_empty: Vec<&&str> = values.iter().filter(|v| !v.is_empty()).collect();
    if non_empty.is_empty() {
        return ColumnType::String;
    }
    if non_empty.iter().all(|v| v.parse::<i64>().is_ok()) {
        return ColumnType::Integer;
    }
    if non_empty.iter().all(|v| v.parse::<f64>().is_ok()) {
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

/// Infers a schema contract from an already-loaded sample.
pub fn infer_schema(sample: &CsvSample) -> SchemaContract {
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
            ColumnSchema {
                name: name.clone(),
                inferred_type: infer_column_type(&values),
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
        .map(|s| ColumnSchema {
            name: s.name,
            inferred_type: column_type_from_parquet_physical_type(&s.physical_type),
        })
        .collect();

    Ok(SchemaContract {
        version: CONTRACT_VERSION.to_string(),
        columns,
    })
}

fn unsupported_extension_error(path: &Path) -> Error {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    Error::from(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("unsupported file extension \"{ext}\" (expected .csv or .parquet)"),
    ))
}

/// Reads a local data file and infers its schema contract in one step.
/// Dispatches on the file extension: `.csv` samples up to `sample_rows` rows
/// and infers types from their string values; `.parquet` reads the file's
/// own embedded schema instead, so `sample_rows` is unused for it.
pub fn infer_schema_from_path(path: &Path, sample_rows: usize) -> Result<SchemaContract> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("csv") => {
            let sample = triplex_io::read_csv_sample(path, sample_rows)?;
            Ok(infer_schema(&sample))
        }
        Some("parquet") => infer_schema_from_parquet(path),
        _ => Err(unsupported_extension_error(path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name)
    }

    #[test]
    fn infers_each_primitive_type() {
        let sample = CsvSample {
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
    }

    #[test]
    fn parquet_null_counts_come_from_footer_statistics_not_a_full_scan() {
        let stats = triplex_io::read_parquet_schema(&fixture("sample.parquet"))
            .expect("parquet fixture should be readable");
        let price = stats.iter().find(|s| s.name == "price").unwrap();
        assert_eq!(price.null_count, 1);
        assert_eq!(price.total_count, 4);
    }

    #[test]
    fn an_unsupported_extension_is_a_clear_error_not_a_panic() {
        let result = infer_schema_from_path(Path::new("data.xlsx"), 100);
        assert!(result.is_err());
    }

    #[test]
    fn reads_and_infers_from_the_synthetic_fixture() {
        let contract = infer_schema_from_path(&fixture("sample.csv"), 100)
            .expect("fixture should be readable");

        assert_eq!(
            contract.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            vec!["id", "price", "in_stock", "name"]
        );
        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[2].inferred_type, ColumnType::Boolean);
        assert_eq!(contract.columns[3].inferred_type, ColumnType::String);
    }
}
