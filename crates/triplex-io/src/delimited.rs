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

/// Writes a header plus rows to a local file as delimited text.
pub(crate) fn write(
    path: &Path,
    delimiter: u8,
    headers: &[String],
    rows: &[Vec<String>],
    wrap_err: fn(String) -> Error,
) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_path(path)
        .map_err(|e| wrap_err(e.to_string()))?;
    writer
        .write_record(headers)
        .map_err(|e| wrap_err(e.to_string()))?;
    for row in rows {
        writer.write_record(row).map_err(|e| wrap_err(e.to_string()))?;
    }
    writer.flush()?;
    Ok(())
}

/// Byte-buffer counterpart to `write` — same serialization, destination is
/// an in-memory buffer (e.g. a masked dataset offered as a browser
/// download in `apps/triplex-web`) instead of a local path.
pub(crate) fn write_bytes(
    delimiter: u8,
    headers: &[String],
    rows: &[Vec<String>],
    wrap_err: fn(String) -> Error,
) -> Result<Vec<u8>> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(Vec::new());
    writer
        .write_record(headers)
        .map_err(|e| wrap_err(e.to_string()))?;
    for row in rows {
        writer
            .write_record(row)
            .map_err(|e| wrap_err(e.to_string()))?;
    }
    writer
        .into_inner()
        .map_err(|e| wrap_err(e.to_string()))
}

pub(crate) fn read_full(path: &Path, delimiter: u8, wrap_err: fn(String) -> Error) -> Result<Sample> {
    let reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .from_path(path)
        .map_err(|e| wrap_err(e.to_string()))?;
    read_full_from(reader, wrap_err)
}

/// Byte-slice counterpart to `read_full` — same parsing, source is data
/// already in memory (e.g. a browser file upload in `apps/triplex-web`)
/// instead of a local path.
pub(crate) fn read_full_bytes(
    bytes: &[u8],
    delimiter: u8,
    wrap_err: fn(String) -> Error,
) -> Result<Sample> {
    let reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .from_reader(bytes);
    read_full_from(reader, wrap_err)
}

fn read_full_from<R: std::io::Read>(
    mut reader: csv::Reader<R>,
    wrap_err: fn(String) -> Error,
) -> Result<Sample> {
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
