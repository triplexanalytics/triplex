use std::path::Path;
use triplex_core::{Error, Result};

/// Row-major and fully owned (`max_rows × columns` individually heap-allocated
/// `String`s) — fine at today's fixed `sample_rows` default (~1000: bounded
/// and fast regardless of total file size, see the CLI's large-file test),
/// but this would need a streaming or column-major rework before either (a)
/// `max_rows` is exposed as a large/unbounded user-facing knob, or (b) a
/// full-file scan mode is added. Flagged for a performance pass before either
/// of those lands, not before.
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

    let mut rows = Vec::with_capacity(max_rows);
    for record in reader.records().take(max_rows) {
        let record = record.map_err(|e| Error::Csv(e.to_string()))?;
        rows.push(record.iter().map(String::from).collect());
    }

    Ok(CsvSample { headers, rows })
}
