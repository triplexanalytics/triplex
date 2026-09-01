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

/// Reads a CSV header plus every data row — for callers that must touch the
/// whole file (like Anonymizer tokenizing real cells), unlike
/// `read_csv_sample`'s bounded sample used for schema inference. Loads the
/// whole file into memory; see `CsvSample`'s doc comment for the scaling
/// ceiling this shares.
pub fn read_csv_full(path: &Path) -> Result<CsvSample> {
    let mut reader = csv::Reader::from_path(path).map_err(|e| Error::Csv(e.to_string()))?;

    let headers = reader
        .headers()
        .map_err(|e| Error::Csv(e.to_string()))?
        .iter()
        .map(String::from)
        .collect();

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| Error::Csv(e.to_string()))?;
        rows.push(record.iter().map(String::from).collect());
    }

    Ok(CsvSample { headers, rows })
}

/// Writes a CSV header plus rows to a local file.
pub fn write_csv(path: &Path, headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let mut writer = csv::Writer::from_path(path).map_err(|e| Error::Csv(e.to_string()))?;
    writer
        .write_record(headers)
        .map_err(|e| Error::Csv(e.to_string()))?;
    for row in rows {
        writer.write_record(row).map_err(|e| Error::Csv(e.to_string()))?;
    }
    writer.flush()?;
    Ok(())
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
}
