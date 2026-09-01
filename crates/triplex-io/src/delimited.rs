//! Shared parsing logic for every delimited-text format (CSV, TSV, ...).
//! Not itself a public format: `csv_source.rs` and `tsv_source.rs` each wrap
//! this with their own delimiter byte and `Error` variant, so a TSV problem
//! is reported as `TSV_PARSE_ERROR`, not `CSV_PARSE_ERROR`, despite sharing
//! this implementation.

use crate::Sample;
use std::path::Path;
use triplex_core::{Error, Result};

pub(crate) fn read_sample(
    path: &Path,
    delimiter: u8,
    max_rows: usize,
    wrap_err: fn(String) -> Error,
) -> Result<Sample> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .from_path(path)
        .map_err(|e| wrap_err(e.to_string()))?;

    let headers = reader
        .headers()
        .map_err(|e| wrap_err(e.to_string()))?
        .iter()
        .map(String::from)
        .collect();

    let mut rows = Vec::with_capacity(max_rows);
    for record in reader.records().take(max_rows) {
        let record = record.map_err(|e| wrap_err(e.to_string()))?;
        rows.push(record.iter().map(String::from).collect());
    }

    Ok(Sample { headers, rows })
}

pub(crate) fn read_full(path: &Path, delimiter: u8, wrap_err: fn(String) -> Error) -> Result<Sample> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .from_path(path)
        .map_err(|e| wrap_err(e.to_string()))?;

    let headers = reader
        .headers()
        .map_err(|e| wrap_err(e.to_string()))?
        .iter()
        .map(String::from)
        .collect();

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| wrap_err(e.to_string()))?;
        rows.push(record.iter().map(String::from).collect());
    }

    Ok(Sample { headers, rows })
}
