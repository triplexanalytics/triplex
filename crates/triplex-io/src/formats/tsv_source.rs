use crate::formats::format::SampleFormat;
use crate::Sample;

use super::delimited;
use std::path::Path;
use triplex_core::{Error, Result};

/// The TSV `SampleFormat` strategy — see `crate::format` for the pattern.
pub struct TsvFormat;

impl SampleFormat for TsvFormat {
    fn read_sample(&self, path: &Path, max_rows: usize) -> Result<Sample> {
        read_tsv_sample(path, max_rows)
    }

    fn read_full(&self, path: &Path) -> Result<Sample> {
        read_tsv_full(path)
    }

    fn read_full_bytes(&self, bytes: &[u8]) -> Result<Sample> {
        read_tsv_full_bytes(bytes)
    }
}

/// Reads a TSV header plus up to `max_rows` data rows from a local file —
/// otherwise identical to `csv_source::read_csv_sample`, both built on the
/// shared `delimited` parsing with just a different delimiter byte.
pub fn read_tsv_sample(path: &Path, max_rows: usize) -> Result<Sample> {
    delimited::read_sample(path, b'\t', max_rows, Error::Tsv)
}

/// Reads a TSV header plus every data row — see `csv_source::read_csv_full`.
pub fn read_tsv_full(path: &Path) -> Result<Sample> {
    delimited::read_full(path, b'\t', Error::Tsv)
}

/// Reads a TSV header plus every data row from an in-memory byte slice —
/// see `csv_source::read_csv_full_bytes`.
pub fn read_tsv_full_bytes(bytes: &[u8]) -> Result<Sample> {
    delimited::read_full_bytes(bytes, b'\t', Error::Tsv)
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
    fn read_tsv_sample_parses_tab_delimited_rows() {
        let sample =
            read_tsv_sample(&fixture("sample.tsv"), 100).expect("fixture should be readable");
        assert_eq!(sample.headers, vec!["id", "price", "in_stock", "name"]);
        assert_eq!(sample.rows[0], vec!["1", "9.99", "true", "widget"]);
    }

    #[test]
    fn tsv_parse_errors_are_labeled_as_tsv_not_csv() {
        let result = read_tsv_sample(&fixture("does_not_exist.tsv"), 100);
        match result {
            Err(e) => assert_eq!(e.code(), "TSV_PARSE_ERROR"),
            Ok(_) => panic!("expected an error for a missing file"),
        }
    }

    #[test]
    fn read_tsv_full_bytes_matches_the_path_based_reader() {
        let bytes = std::fs::read(fixture("sample.tsv")).unwrap();
        let from_bytes = read_tsv_full_bytes(&bytes).expect("bytes should parse");
        let from_path = read_tsv_full(&fixture("sample.tsv")).unwrap();
        assert_eq!(from_bytes.headers, from_path.headers);
        assert_eq!(from_bytes.rows, from_path.rows);
    }
}
