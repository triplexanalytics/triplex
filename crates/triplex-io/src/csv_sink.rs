use crate::delimited;
use std::path::Path;
use triplex_core::{Error, Result};

const COMMA: u8 = b',';

/// Writes a CSV header plus rows to a local file.
pub fn write_csv(path: &Path, headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    write_csv_with_delimiter(path, COMMA, headers, rows)
}

/// Writes a header plus rows using a custom field separator instead of a
/// comma — see `csv_source::read_csv_sample_with_delimiter`.
pub fn write_csv_with_delimiter(
    path: &Path,
    delimiter: u8,
    headers: &[String],
    rows: &[Vec<String>],
) -> Result<()> {
    delimited::write(path, delimiter, headers, rows, Error::Csv)
}

/// Writes a CSV header plus rows to an in-memory byte buffer instead of a
/// local file.
pub fn write_csv_bytes(headers: &[String], rows: &[Vec<String>]) -> Result<Vec<u8>> {
    delimited::write_bytes(COMMA, headers, rows, Error::Csv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv_source::{read_csv_full, read_csv_full_with_delimiter};

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
    fn write_csv_bytes_round_trips_through_read_csv_full_bytes() {
        let headers = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec!["1".to_string(), "x".to_string()]];

        let bytes = write_csv_bytes(&headers, &rows).expect("write should succeed");
        let read_back =
            crate::csv_source::read_csv_full_bytes(&bytes).expect("read should succeed");

        assert_eq!(read_back.headers, headers);
        assert_eq!(read_back.rows, rows);
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
        let read_back = read_csv_full_with_delimiter(&path, b'|')
            .expect("read with the same delimiter should succeed");
        std::fs::remove_file(&path).ok();

        assert_eq!(read_back.headers, headers);
        assert_eq!(read_back.rows, rows);
    }
}
