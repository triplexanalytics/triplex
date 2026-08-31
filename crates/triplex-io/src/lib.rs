//! Local file I/O. No network calls. Reads customer data only from a local path
//! the caller provides, and never transmits it anywhere.

use std::fs::File;
use std::path::Path;
use triplex_core::{Error, Result};

pub struct CsvSample {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Reads a CSV header plus up to `max_rows` data rows from a local file.
pub fn read_csv_sample(path: &Path, max_rows: usize) -> Result<CsvSample> {
    let mut reader = csv::Reader::from_path(path).map_err(|e| Error::Csv(e.to_string()))?;

    let headers = reader
        .headers()
        .map_err(|e| Error::Csv(e.to_string()))?
        .iter()
        .map(String::from)
        .collect();

    let mut rows = Vec::new();
    for record in reader.records().take(max_rows) {
        let record = record.map_err(|e| Error::Csv(e.to_string()))?;
        rows.push(record.iter().map(String::from).collect());
    }

    Ok(CsvSample { headers, rows })
}

pub struct ParquetColumnStats {
    pub name: String,
    /// The Parquet physical type as its Display string (e.g. "INT32", "BYTE_ARRAY"),
    /// deferring type mapping to the caller rather than coupling this crate to
    /// triplex-schemacast's ColumnType.
    pub physical_type: String,
    pub null_count: u64,
    pub total_count: u64,
}

/// Reads column names, physical types, and null-count statistics from a local
/// Parquet file's footer metadata only — the row data itself is never decoded,
/// so this stays cheap regardless of file size.
pub fn read_parquet_schema(path: &Path) -> Result<Vec<ParquetColumnStats>> {
    use parquet::file::reader::{FileReader, SerializedFileReader};

    let file = File::open(path)?;
    let reader =
        SerializedFileReader::new(file).map_err(|e| Error::Parquet(e.to_string()))?;
    let metadata = reader.metadata();
    let schema = metadata.file_metadata().schema_descr();

    let mut stats: Vec<ParquetColumnStats> = schema
        .columns()
        .iter()
        .map(|col| ParquetColumnStats {
            name: col.name().to_string(),
            physical_type: col.physical_type().to_string(),
            null_count: 0,
            total_count: 0,
        })
        .collect();

    for row_group in metadata.row_groups() {
        let num_rows = row_group.num_rows().max(0) as u64;
        for (i, stat) in stats.iter_mut().enumerate() {
            stat.total_count += num_rows;
            if let Some(column_stats) = row_group.column(i).statistics() {
                stat.null_count += column_stats.null_count_opt().unwrap_or(0);
            }
        }
    }

    Ok(stats)
}
