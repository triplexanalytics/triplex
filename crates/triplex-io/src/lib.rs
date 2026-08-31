//! Local file I/O. No network calls. Reads customer data only from a local path
//! the caller provides, and never transmits it anywhere.

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
