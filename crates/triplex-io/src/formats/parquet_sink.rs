//! Writing a typed Parquet file. Deliberately its own file from
//! `parquet_source.rs` — reading Parquet's footer/row data and writing a
//! fresh file are different enough concerns (and `parquet_source.rs` is
//! already sizeable) that keeping them apart is clearer than one large file.

use arrow_array::{ArrayRef, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use triplex_core::{Error, Result};

/// The physical type to write a column as. Deliberately not
/// `triplex_schemacast::ColumnType` — triplex-io must not depend on a crate
/// that depends on it (schemacast depends on io, not the reverse); the
/// caller maps its own type enum onto this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteColumnType {
    Integer,
    Float,
    Boolean,
    String,
}

fn parse_or_none<T: std::str::FromStr>(value: &str) -> Option<T> {
    if value.is_empty() {
        None
    } else {
        triplex_core::normalize_numeric_string(value).parse().ok()
    }
}

/// Builds one Arrow array from a column's string values, coercing each into
/// `column_type`. A value that doesn't parse as that type (a real mismatch,
/// not just an empty cell) becomes `null` rather than failing the whole
/// write — the caller is expected to have already inferred `column_type`
/// from this same data, so mismatches should be rare, but a stray bad value
/// shouldn't block writing everything else. `pub(crate)`: also used by
/// `arrow_ipc_sink.rs`, the other Arrow-array-building consumer.
pub(crate) fn build_array(column_type: WriteColumnType, values: &[&str]) -> ArrayRef {
    match column_type {
        WriteColumnType::Integer => Arc::new(Int64Array::from(
            values
                .iter()
                .map(|v| parse_or_none::<i64>(v))
                .collect::<Vec<_>>(),
        )),
        WriteColumnType::Float => Arc::new(Float64Array::from(
            values
                .iter()
                .map(|v| parse_or_none::<f64>(v))
                .collect::<Vec<_>>(),
        )),
        WriteColumnType::Boolean => Arc::new(BooleanArray::from(
            values
                .iter()
                .map(|v| (!v.is_empty()).then(|| v.eq_ignore_ascii_case("true")))
                .collect::<Vec<_>>(),
        )),
        WriteColumnType::String => Arc::new(StringArray::from(
            values
                .iter()
                .map(|v| (!v.is_empty()).then_some(*v))
                .collect::<Vec<_>>(),
        )),
    }
}

/// Builds the Arrow `Schema` for `headers`/`column_types` — every field
/// nullable, matching how this crate treats a missing/mismatched cell as
/// `null` rather than failing the whole column. `pub(crate)`: shared with
/// `arrow_ipc_sink.rs`, so a Parquet file and an IPC stream built from the
/// same rows always agree on field types.
pub(crate) fn schema_for(headers: &[String], column_types: &[WriteColumnType]) -> Arc<Schema> {
    let fields: Vec<Field> = headers
        .iter()
        .zip(column_types)
        .map(|(name, column_type)| {
            let data_type = match column_type {
                WriteColumnType::Integer => DataType::Int64,
                WriteColumnType::Float => DataType::Float64,
                WriteColumnType::Boolean => DataType::Boolean,
                WriteColumnType::String => DataType::Utf8,
            };
            Field::new(name, data_type, true)
        })
        .collect();
    Arc::new(Schema::new(fields))
}

/// Builds one Arrow array per column from `rows`, in `column_types` order.
/// `pub(crate)`: shared with `arrow_ipc_sink.rs`.
pub(crate) fn arrays_for(column_types: &[WriteColumnType], rows: &[Vec<String>]) -> Vec<ArrayRef> {
    column_types
        .iter()
        .enumerate()
        .map(|(col_idx, column_type)| {
            let values: Vec<&str> = rows.iter().map(|row| row[col_idx].as_str()).collect();
            build_array(*column_type, &values)
        })
        .collect()
}

/// Writes a typed Parquet file: `column_types[i]` is the physical type for
/// `headers[i]`. Every column is nullable — an empty cell becomes Parquet
/// `null`, matching how the rest of this crate treats a missing value.
pub fn write_parquet(
    path: &Path,
    headers: &[String],
    column_types: &[WriteColumnType],
    rows: &[Vec<String>],
) -> Result<()> {
    let schema = schema_for(headers, column_types);
    let arrays = arrays_for(column_types, rows);

    let batch =
        RecordBatch::try_new(schema.clone(), arrays).map_err(|e| Error::Parquet(e.to_string()))?;

    let file = File::create(path)?;
    let mut writer =
        ArrowWriter::try_new(file, schema, None).map_err(|e| Error::Parquet(e.to_string()))?;
    writer
        .write(&batch)
        .map_err(|e| Error::Parquet(e.to_string()))?;
    writer.close().map_err(|e| Error::Parquet(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_parquet_rows;

    #[test]
    fn writes_and_reads_back_a_typed_column_of_each_kind() {
        let path = std::env::temp_dir().join(format!(
            "triplex_io_write_parquet_{}.parquet",
            std::process::id()
        ));

        let headers = vec![
            "id".to_string(),
            "price".to_string(),
            "in_stock".to_string(),
            "name".to_string(),
        ];
        let column_types = [
            WriteColumnType::Integer,
            WriteColumnType::Float,
            WriteColumnType::Boolean,
            WriteColumnType::String,
        ];
        let rows = vec![
            vec!["1".into(), "9.99".into(), "true".into(), "widget".into()],
            vec!["2".into(), "".into(), "false".into(), "gadget".into()],
        ];

        write_parquet(&path, &headers, &column_types, &rows).expect("write should succeed");
        let read_back = read_parquet_rows(&path).expect("read should succeed");
        std::fs::remove_file(&path).ok();

        assert_eq!(read_back.headers, headers);
        assert_eq!(read_back.rows[0], vec!["1", "9.99", "true", "widget"]);
        // The empty price cell round-trips as Parquet null, which
        // read_parquet_rows renders back as an empty string.
        assert_eq!(read_back.rows[1], vec!["2", "", "false", "gadget"]);
    }
}
