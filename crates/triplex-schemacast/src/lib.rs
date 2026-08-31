//! Structural firewall: infers a schema contract from a local data sample and
//! emits it as versioned, deterministic JSON. No network calls; the caller's
//! path is the only source of data, and nothing here transmits it anywhere.

use serde::Serialize;
use std::path::Path;
use triplex_core::Result;
use triplex_io::CsvSample;

const CONTRACT_VERSION: &str = "0.1.0";

#[derive(Debug, PartialEq, Eq, Serialize)]
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

/// Reads a local CSV file and infers its schema contract in one step.
pub fn infer_schema_from_path(path: &Path, sample_rows: usize) -> Result<SchemaContract> {
    let sample = triplex_io::read_csv_sample(path, sample_rows)?;
    Ok(infer_schema(&sample))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn reads_and_infers_from_the_synthetic_fixture() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/sample.csv");

        let contract = infer_schema_from_path(&path, 100).expect("fixture should be readable");

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
