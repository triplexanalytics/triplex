//! Serializing rows to Arrow's IPC stream format: same typed-array
//! construction as `parquet_sink.rs` (shares its `schema_for`/`arrays_for`/
//! `build_array`), different output — an in-memory `Vec<u8>` a JS Arrow
//! reader can decode directly (e.g. `apache-arrow`'s `tableFromIPC`), not a
//! file. Built for `apps/triplex-web`'s full-file table view: typed
//! columnar data across the wasm/JS boundary instead of a giant JSON string
//! of stringified cells — see the design notes.

use super::parquet_sink::{arrays_for, schema_for, WriteColumnType};
use arrow_array::RecordBatch;
use arrow_ipc::writer::StreamWriter;
use triplex_core::{Error, Result};

/// `column_types[i]` should already be known-safe for every non-empty value
/// in column `i` — e.g. schemacast's `infer_schema` only classifies a
/// column `Integer`/`Float` when every non-empty cell already parses
/// cleanly as that type, so `build_array`'s per-value fallback to `null`
/// (see its doc comment) should never actually fire here in practice.
pub fn rows_to_arrow_ipc(
    headers: &[String],
    column_types: &[WriteColumnType],
    rows: &[Vec<String>],
) -> Result<Vec<u8>> {
    let schema = schema_for(headers, column_types);
    let arrays = arrays_for(column_types, rows);
    let batch =
        RecordBatch::try_new(schema.clone(), arrays).map_err(|e| Error::Arrow(e.to_string()))?;

    let mut buf = Vec::new();
    {
        let mut writer =
            StreamWriter::try_new(&mut buf, &schema).map_err(|e| Error::Arrow(e.to_string()))?;
        writer
            .write(&batch)
            .map_err(|e| Error::Arrow(e.to_string()))?;
        writer.finish().map_err(|e| Error::Arrow(e.to_string()))?;
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::Array;
    use arrow_ipc::reader::StreamReader;
    use std::io::Cursor;

    #[test]
    fn round_trips_headers_and_typed_values_through_ipc_bytes() {
        let headers = vec!["id".to_string(), "price".to_string(), "name".to_string()];
        let column_types = vec![
            WriteColumnType::Integer,
            WriteColumnType::Float,
            WriteColumnType::String,
        ];
        let rows = vec![
            vec!["1".to_string(), "9.5".to_string(), "widget".to_string()],
            vec!["2".to_string(), "".to_string(), "gadget".to_string()],
        ];

        let bytes = rows_to_arrow_ipc(&headers, &column_types, &rows).unwrap();

        let mut reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
        let schema = reader.schema();
        let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        assert_eq!(names, headers);

        let batch = reader.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);

        let ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<arrow_array::Int64Array>()
            .unwrap();
        assert_eq!(ids.value(0), 1);
        assert_eq!(ids.value(1), 2);

        let prices = batch
            .column(1)
            .as_any()
            .downcast_ref::<arrow_array::Float64Array>()
            .unwrap();
        assert_eq!(prices.value(0), 9.5);
        assert!(
            prices.is_null(1),
            "an empty cell must round-trip as Arrow null, not 0.0"
        );

        let names_col = batch
            .column(2)
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .unwrap();
        assert_eq!(names_col.value(0), "widget");
        assert_eq!(names_col.value(1), "gadget");

        assert!(reader.next().is_none(), "only one batch was written");
    }
}
