//! Reading an Arrow IPC stream into a `Sample`: the inverse of
//! `arrow_ipc_sink.rs`, for a caller that already holds a typed table in
//! memory (`packages/triplex-python` handing over a DataFrame) rather than a
//! file. Cells become strings like every other source here, since that is
//! what the engine works on; the typed columns are not kept.

use crate::Sample;
use arrow_array::{Array, RecordBatch};
use arrow_cast::display::array_value_to_string;
use arrow_ipc::reader::StreamReader;
use arrow_schema::ArrowError;
use triplex_core::{Error, Result};

/// Appends every row of `batch` to `rows` as strings, a null cell as the
/// empty string. Shared with `parquet_source.rs`, which decodes to the same
/// Arrow batches.
pub(crate) fn append_rows(
    batch: &RecordBatch,
    rows: &mut Vec<Vec<String>>,
) -> std::result::Result<(), ArrowError> {
    for row_idx in 0..batch.num_rows() {
        let mut row = Vec::with_capacity(batch.num_columns());
        for array in batch.columns() {
            row.push(if array.is_null(row_idx) {
                String::new()
            } else {
                array_value_to_string(array, row_idx)?
            });
        }
        rows.push(row);
    }
    Ok(())
}

/// Reads a header plus every data row from Arrow IPC stream bytes.
pub fn read_arrow_ipc_full_bytes(bytes: &[u8]) -> Result<Sample> {
    let arrow = |e: ArrowError| Error::Arrow(e.to_string());
    let reader = StreamReader::try_new(bytes, None).map_err(arrow)?;
    let headers = reader
        .schema()
        .fields()
        .iter()
        .map(|f| f.name().clone())
        .collect();
    let mut rows = Vec::new();
    for batch in reader {
        append_rows(&batch.map_err(arrow)?, &mut rows).map_err(arrow)?;
    }
    Ok(Sample { headers, rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rows_to_arrow_ipc, WriteColumnType};

    #[test]
    fn round_trips_through_the_ipc_sink_with_a_null_as_the_empty_string() {
        let headers = vec!["id".to_string(), "price".to_string(), "name".to_string()];
        let types = [
            WriteColumnType::Integer,
            WriteColumnType::Float,
            WriteColumnType::String,
        ];
        let rows = vec![
            vec!["1".to_string(), "9.5".to_string(), "widget".to_string()],
            vec!["2".to_string(), String::new(), "gadget".to_string()],
        ];
        let bytes = rows_to_arrow_ipc(&headers, &types, &rows).unwrap();

        let sample = read_arrow_ipc_full_bytes(&bytes).unwrap();

        assert_eq!(sample.headers, headers);
        assert_eq!(sample.rows, rows);
    }

    #[test]
    fn bytes_that_are_not_an_ipc_stream_are_an_arrow_error() {
        let err = read_arrow_ipc_full_bytes(b"id,price\n1,2\n").err().unwrap();
        assert_eq!(err.code(), "ARROW_ERROR");
    }
}
