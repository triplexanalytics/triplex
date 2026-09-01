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
