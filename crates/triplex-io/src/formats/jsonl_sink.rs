use std::fs::File;
use std::io::Write as _;
use std::path::Path;
use triplex_core::{Error, Result};

/// Writes one JSON object per row (keys = headers) to a local file. Every
/// value is written as a JSON string — the pipeline is string-based end to
/// end, so this can't tell a would-be number like `"1"` from a genuine
/// string `"1"` on the way out. Round-tripping through JSONL therefore
/// preserves structure and text, not JSON's native scalar types.
pub fn write_jsonl(path: &Path, headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let mut file = File::create(path)?;
    for row in rows {
        let line = row_to_json_line(headers, row)?;
        writeln!(file, "{line}")?;
    }
    Ok(())
}

/// Writes one JSON object per row to an in-memory byte buffer instead of a
/// local file — see `write_jsonl`.
pub fn write_jsonl_bytes(headers: &[String], rows: &[Vec<String>]) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    for row in rows {
        let line = row_to_json_line(headers, row)?;
        writeln!(buf, "{line}")?;
    }
    Ok(buf)
}

fn row_to_json_line(headers: &[String], row: &[String]) -> Result<String> {
    let mut object = serde_json::Map::new();
    for (header, value) in headers.iter().zip(row) {
        object.insert(header.clone(), serde_json::Value::String(value.clone()));
    }
    serde_json::to_string(&serde_json::Value::Object(object))
        .map_err(|e| Error::Jsonl(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::super::jsonl_source::{read_jsonl_full, read_jsonl_full_bytes};
    use super::*;

    #[test]
    fn write_jsonl_bytes_round_trips_through_read_jsonl_full_bytes() {
        let headers = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec!["1".to_string(), "x".to_string()]];

        let bytes = write_jsonl_bytes(&headers, &rows).expect("write should succeed");
        let read_back = read_jsonl_full_bytes(&bytes).expect("read should succeed");

        assert_eq!(read_back.headers, headers);
        assert_eq!(read_back.rows, rows);
    }

    #[test]
    fn write_jsonl_round_trips_through_read_jsonl_full() {
        let path = std::env::temp_dir().join(format!(
            "triplex_io_write_jsonl_roundtrip_{}.jsonl",
            std::process::id()
        ));
        let headers = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec!["1".to_string(), "x".to_string()]];

        write_jsonl(&path, &headers, &rows).expect("write should succeed");
        let read_back = read_jsonl_full(&path).expect("read should succeed");
        std::fs::remove_file(&path).ok();

        assert_eq!(read_back.headers, headers);
        assert_eq!(read_back.rows, rows);
    }
}
