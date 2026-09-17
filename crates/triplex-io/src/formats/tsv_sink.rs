use std::path::Path;

use super::delimited;
use triplex_core::{Error, Result};

/// Writes a TSV header plus rows to a local file.
pub fn write_tsv(path: &Path, headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    delimited::write(path, b'\t', headers, rows, Error::Tsv)
}

/// Writes a TSV header plus rows to an in-memory byte buffer.
pub fn write_tsv_bytes(headers: &[String], rows: &[Vec<String>]) -> Result<Vec<u8>> {
    delimited::write_bytes(b'\t', headers, rows, Error::Tsv)
}

#[cfg(test)]
mod tests {
    use super::super::tsv_source::{read_tsv_full, read_tsv_full_bytes};
    use super::*;

    #[test]
    fn write_tsv_bytes_round_trips_through_read_tsv_full_bytes() {
        let headers = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec!["1".to_string(), "x".to_string()]];

        let bytes = write_tsv_bytes(&headers, &rows).expect("write should succeed");
        let read_back = read_tsv_full_bytes(&bytes).expect("read should succeed");

        assert_eq!(read_back.headers, headers);
        assert_eq!(read_back.rows, rows);
    }

    #[test]
    fn write_tsv_round_trips_through_read_tsv_full() {
        let path = std::env::temp_dir().join(format!(
            "triplex_io_write_tsv_roundtrip_{}.tsv",
            std::process::id()
        ));
        let headers = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec!["1".to_string(), "x".to_string()]];

        write_tsv(&path, &headers, &rows).expect("write should succeed");
        let read_back = read_tsv_full(&path).expect("read should succeed");
        std::fs::remove_file(&path).ok();

        assert_eq!(read_back.headers, headers);
        assert_eq!(read_back.rows, rows);
    }
}
