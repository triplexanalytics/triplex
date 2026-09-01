use crate::statistics::{ColumnStatistics, ColumnStatisticsBuilder};
use std::fs::File;
use std::path::Path;
use triplex_core::{Error, Result};

pub struct ParquetColumnStats {
    pub name: String,
    /// The Parquet physical type as its Display string (e.g. "INT32", "BYTE_ARRAY"),
    /// deferring type mapping to the caller rather than coupling this crate to
    /// triplex-schemacast's ColumnType.
    pub physical_type: String,
    pub statistics: ColumnStatistics,
}

/// Extracts typed min/max as f64 from a row group's per-column statistics.
/// Only numeric physical types carry a meaningful range; everything else
/// (booleans, byte arrays) returns `None`.
fn numeric_min_max(stats: &parquet::file::statistics::Statistics) -> (Option<f64>, Option<f64>) {
    use parquet::file::statistics::Statistics;
    match stats {
        Statistics::Int32(s) => (
            s.min_opt().map(|v| *v as f64),
            s.max_opt().map(|v| *v as f64),
        ),
        Statistics::Int64(s) => (
            s.min_opt().map(|v| *v as f64),
            s.max_opt().map(|v| *v as f64),
        ),
        Statistics::Float(s) => (
            s.min_opt().map(|v| *v as f64),
            s.max_opt().map(|v| *v as f64),
        ),
        Statistics::Double(s) => (s.min_opt().copied(), s.max_opt().copied()),
        _ => (None, None),
    }
}

/// Reads column names, physical types, and statistics from a local Parquet
/// file's footer metadata only — the row data itself is never decoded, so
/// this stays cheap regardless of file size. `mean_value` is always `None`
/// (see `ColumnStatistics`).
pub fn read_parquet_schema(path: &Path) -> Result<Vec<ParquetColumnStats>> {
    use parquet::file::reader::{FileReader, SerializedFileReader};

    let file = File::open(path)?;
    let reader =
        SerializedFileReader::new(file).map_err(|e| Error::Parquet(e.to_string()))?;
    let metadata = reader.metadata();
    let schema = metadata.file_metadata().schema_descr();

    // Only name/physical_type are genuinely Parquet-schema-specific here;
    // all null/min/max accumulation is delegated to ColumnStatisticsBuilder
    // so this file doesn't reimplement that merge logic itself.
    struct ColumnAccumulator {
        name: String,
        physical_type: String,
        builder: ColumnStatisticsBuilder,
    }

    let mut accumulators: Vec<ColumnAccumulator> = schema
        .columns()
        .iter()
        .map(|col| ColumnAccumulator {
            name: col.name().to_string(),
            physical_type: col.physical_type().to_string(),
            builder: ColumnStatisticsBuilder::new(),
        })
        .collect();

    for row_group in metadata.row_groups() {
        let num_rows = row_group.num_rows().max(0) as u64;
        for (i, acc) in accumulators.iter_mut().enumerate() {
            let column_stats = row_group.column(i).statistics();
            let null_count = column_stats.map_or(0, |s| s.null_count_opt().unwrap_or(0));
            let (min, max) = column_stats.map_or((None, None), numeric_min_max);
            acc.builder.record_batch(null_count, num_rows, min, max);
        }
    }

    Ok(accumulators
        .into_iter()
        .map(|acc| ParquetColumnStats {
            name: acc.name,
            physical_type: acc.physical_type,
            statistics: acc.builder.build(),
        })
        .collect())
}

/// Reads every row of a local Parquet file, decoding actual column values
/// (not just footer statistics) into strings. Unlike `read_parquet_schema`'s
/// zero-copy metadata-only read, this is for callers that must touch every
/// value — e.g. Anonymizer tokenizing real cells. Loads the whole decoded
/// dataset into memory; revisit with a batch-by-batch streaming writer if
/// anonymizing very large Parquet files becomes a real use case.
pub fn read_parquet_rows(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    use arrow_cast::display::array_value_to_string;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let file = File::open(path)?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| Error::Parquet(e.to_string()))?;
    let headers: Vec<String> = builder
        .schema()
        .fields()
        .iter()
        .map(|f| f.name().clone())
        .collect();
    let reader = builder.build().map_err(|e| Error::Parquet(e.to_string()))?;

    let mut rows = Vec::new();
    for batch_result in reader {
        let batch = batch_result.map_err(|e| Error::Parquet(e.to_string()))?;
        for row_idx in 0..batch.num_rows() {
            let mut row = Vec::with_capacity(batch.num_columns());
            for col_idx in 0..batch.num_columns() {
                let array = batch.column(col_idx);
                let value = if array.is_null(row_idx) {
                    String::new()
                } else {
                    array_value_to_string(array, row_idx)
                        .map_err(|e| Error::Parquet(e.to_string()))?
                };
                row.push(value);
            }
            rows.push(row);
        }
    }

    Ok((headers, rows))
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
    fn reads_every_row_including_a_null_cell() {
        let (headers, rows) = read_parquet_rows(&fixture("sample.parquet"))
            .expect("parquet fixture should be readable");

        assert_eq!(headers, vec!["id", "price", "in_stock", "name"]);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0], vec!["1", "9.99", "true", "widget"]);
        assert_eq!(rows[1], vec!["2", "3.5", "false", "gadget"]);
        assert_eq!(rows[2], vec!["3", "12.0", "true", "sprocket"]);
        // Row 4's price is null in the fixture.
        assert_eq!(rows[3], vec!["4", "", "true", "doohickey"]);
    }
}
