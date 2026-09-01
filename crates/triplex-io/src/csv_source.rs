use crate::format::SampleFormat;
use crate::{delimited, Sample};
use std::path::Path;
use triplex_core::{Error, Result};

/// The CSV `SampleFormat` strategy — see `crate::format` for the pattern.
pub struct CsvFormat;

impl SampleFormat for CsvFormat {
    fn read_sample(&self, path: &Path, max_rows: usize) -> Result<Sample> {
        read_csv_sample(path, max_rows)
    }

    fn read_full(&self, path: &Path) -> Result<Sample> {
        read_csv_full(path)
    }

    fn read_full_bytes(&self, bytes: &[u8]) -> Result<Sample> {
        read_csv_full_bytes(bytes)
    }
}

const COMMA: u8 = b',';

/// Reads a CSV header plus up to `max_rows` data rows from a local file.
pub fn read_csv_sample(path: &Path, max_rows: usize) -> Result<Sample> {
    read_csv_sample_with_delimiter(path, COMMA, max_rows)
}

/// Reads a CSV header plus every data row — for callers that must touch the
/// whole file (like Anonymizer tokenizing real cells), unlike
/// `read_csv_sample`'s bounded sample used for schema inference. Loads the
/// whole file into memory; see `Sample`'s doc comment for the scaling
/// ceiling this shares.
pub fn read_csv_full(path: &Path) -> Result<Sample> {
    read_csv_full_with_delimiter(path, COMMA)
}

/// Reads a `.csv`-shaped file whose actual field separator isn't a comma —
/// e.g. the semicolon-delimited exports common under European Excel locale
/// settings, or a pipe-delimited feed. Still reported as `CSV_PARSE_ERROR`
/// on failure: the delimiter is a parsing detail, not a different format.
pub fn read_csv_sample_with_delimiter(
    path: &Path,
    delimiter: u8,
    max_rows: usize,
) -> Result<Sample> {
    delimited::read_sample(path, delimiter, max_rows, Error::Csv)
}

/// Full-file counterpart to `read_csv_sample_with_delimiter` — see
/// `read_csv_full`'s doc comment for why a "full" variant exists at all.
pub fn read_csv_full_with_delimiter(path: &Path, delimiter: u8) -> Result<Sample> {
    delimited::read_full(path, delimiter, Error::Csv)
}

/// Reads a CSV header plus every data row from an in-memory byte slice
/// (comma-delimited) — see `crate::delimited::read_full_bytes` for why a
/// bytes variant exists alongside the path-based readers (no filesystem in
/// a browser/WASM caller like `apps/triplex-web`).
pub fn read_csv_full_bytes(bytes: &[u8]) -> Result<Sample> {
    delimited::read_full_bytes(bytes, COMMA, Error::Csv)
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
    fn read_csv_full_reads_every_row_unlike_the_bounded_sample() {
        let full = read_csv_full(&fixture("sample.csv")).expect("fixture should be readable");
        let capped =
            read_csv_sample(&fixture("sample.csv"), 2).expect("fixture should be readable");
        assert_eq!(full.rows.len(), 4);
        assert_eq!(capped.rows.len(), 2);
    }

    #[test]
    fn a_semicolon_delimited_csv_export_reads_correctly_with_an_explicit_delimiter() {
        let sample = read_csv_sample_with_delimiter(&fixture("sample.semicolon.csv"), b';', 100)
            .expect("fixture should be readable with the right delimiter");
        assert_eq!(sample.headers, vec!["id", "price", "in_stock", "name"]);
        assert_eq!(sample.rows[0], vec!["1", "9.99", "true", "widget"]);
    }

    #[test]
    fn read_csv_full_bytes_matches_the_path_based_reader() {
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let from_bytes = read_csv_full_bytes(&bytes).expect("bytes should parse");
        let from_path = read_csv_full(&fixture("sample.csv")).unwrap();
        assert_eq!(from_bytes.headers, from_path.headers);
        assert_eq!(from_bytes.rows, from_path.rows);
    }
}
