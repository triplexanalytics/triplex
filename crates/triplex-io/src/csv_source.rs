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

/// Writes a CSV header plus rows to a local file.
pub fn write_csv(path: &Path, headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    write_csv_with_delimiter(path, COMMA, headers, rows)
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

/// Writes a header plus rows using a custom field separator instead of a
/// comma — see `read_csv_sample_with_delimiter`.
pub fn write_csv_with_delimiter(
    path: &Path,
    delimiter: u8,
    headers: &[String],
    rows: &[Vec<String>],
) -> Result<()> {
    delimited::write(path, delimiter, headers, rows, Error::Csv)
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
        let capped = read_csv_sample(&fixture("sample.csv"), 2).expect("fixture should be readable");
        assert_eq!(full.rows.len(), 4);
        assert_eq!(capped.rows.len(), 2);
    }

    #[test]
    fn write_csv_round_trips_through_read_csv_full() {
        let path = std::env::temp_dir().join(format!(
            "triplex_io_write_csv_roundtrip_{}.csv",
            std::process::id()
        ));
        let headers = vec!["a".to_string(), "b".to_string()];
        let rows = vec![
            vec!["1".to_string(), "x".to_string()],
            vec!["2".to_string(), "y".to_string()],
        ];

        write_csv(&path, &headers, &rows).expect("write should succeed");
        let read_back = read_csv_full(&path).expect("read should succeed");
        std::fs::remove_file(&path).ok();

        assert_eq!(read_back.headers, headers);
        assert_eq!(read_back.rows, rows);
    }

    #[test]
    fn a_semicolon_delimited_csv_export_reads_correctly_with_an_explicit_delimiter() {
        let sample = read_csv_sample_with_delimiter(&fixture("sample.semicolon.csv"), b';', 100)
            .expect("fixture should be readable with the right delimiter");
        assert_eq!(sample.headers, vec!["id", "price", "in_stock", "name"]);
        assert_eq!(sample.rows[0], vec!["1", "9.99", "true", "widget"]);
    }

    #[test]
    fn write_csv_with_delimiter_round_trips_through_the_matching_reader() {
        let path = std::env::temp_dir().join(format!(
            "triplex_io_write_csv_pipe_delimited_{}.csv",
            std::process::id()
        ));
        let headers = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec!["1".to_string(), "x".to_string()]];

        write_csv_with_delimiter(&path, b'|', &headers, &rows).expect("write should succeed");
        let read_back =
            read_csv_full_with_delimiter(&path, b'|').expect("read with the same delimiter should succeed");
        std::fs::remove_file(&path).ok();

        assert_eq!(read_back.headers, headers);
        assert_eq!(read_back.rows, rows);
    }
}
