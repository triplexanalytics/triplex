use crate::format::SampleFormat;
use crate::Sample;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use triplex_core::{Error, Result};

/// The JSONL `SampleFormat` strategy — see `crate::format` for the pattern.
pub struct JsonlFormat;

impl SampleFormat for JsonlFormat {
    fn read_sample(&self, path: &Path, max_rows: usize) -> Result<Sample> {
        read_jsonl_sample(path, max_rows)
    }
}

/// Renders a JSON scalar as the plain string our string-based pipeline
/// expects: a JSON string is used as-is, `null` becomes empty (matching how
/// CSV treats a missing cell), and anything else (number, bool, nested
/// object/array) falls back to its JSON text form.
fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Reads up to `max_rows` JSON objects (one per line) from a local
/// JSONL/NDJSON file. The header is the *first* row's keys, in that row's
/// order — a later row missing one of those keys gets an empty string for
/// it, and a later row with a key the first row didn't have has that key
/// dropped. Full schema-drift union across every sampled row isn't
/// implemented; revisit if JSONL files with inconsistent keys per line turn
/// out to be common.
pub fn read_jsonl_sample(path: &Path, max_rows: usize) -> Result<Sample> {
    let reader = BufReader::new(File::open(path)?);

    let mut headers: Option<Vec<String>> = None;
    let mut rows = Vec::with_capacity(max_rows);

    for line in reader.lines().take(max_rows) {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let value: serde_json::Value =
            serde_json::from_str(&line).map_err(|e| Error::Jsonl(e.to_string()))?;
        let object = value
            .as_object()
            .ok_or_else(|| Error::Jsonl("each line must be a JSON object".to_string()))?;

        let headers = headers.get_or_insert_with(|| object.keys().cloned().collect());
        let row = headers
            .iter()
            .map(|h| object.get(h).map(json_value_to_string).unwrap_or_default())
            .collect();
        rows.push(row);
    }

    Ok(Sample {
        headers: headers.unwrap_or_default(),
        rows,
    })
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
    fn reads_headers_from_the_first_line_and_values_from_every_line() {
        let sample =
            read_jsonl_sample(&fixture("sample.jsonl"), 100).expect("fixture should be readable");
        assert_eq!(sample.headers, vec!["id", "price", "in_stock", "name"]);
        assert_eq!(sample.rows[0], vec!["1", "9.99", "true", "widget"]);
        assert_eq!(sample.rows.len(), 4);
    }

    #[test]
    fn a_null_value_becomes_an_empty_string() {
        let sample =
            read_jsonl_sample(&fixture("sample.jsonl"), 100).expect("fixture should be readable");
        // Row 4's price is JSON null in the fixture.
        assert_eq!(sample.rows[3][1], "");
    }

    #[test]
    fn a_line_missing_a_first_row_key_gets_an_empty_string_for_it() {
        let path = std::env::temp_dir().join(format!(
            "triplex_io_jsonl_missing_key_{}.jsonl",
            std::process::id()
        ));
        std::fs::write(&path, "{\"a\":\"1\",\"b\":\"2\"}\n{\"a\":\"3\"}\n").unwrap();

        let sample = read_jsonl_sample(&path, 100).expect("fixture should be readable");
        std::fs::remove_file(&path).ok();

        assert_eq!(sample.headers, vec!["a", "b"]);
        assert_eq!(sample.rows[1], vec!["3", ""]);
    }

    #[test]
    fn a_line_that_is_not_a_json_object_is_a_clear_error() {
        let path = std::env::temp_dir().join(format!(
            "triplex_io_jsonl_not_an_object_{}.jsonl",
            std::process::id()
        ));
        std::fs::write(&path, "[1, 2, 3]\n").unwrap();

        let result = read_jsonl_sample(&path, 100);
        std::fs::remove_file(&path).ok();

        match result {
            Err(e) => assert_eq!(e.code(), "JSONL_PARSE_ERROR"),
            Ok(_) => panic!("expected an error for a non-object line"),
        }
    }

    #[test]
    fn max_rows_bounds_how_many_lines_are_read() {
        let sample = read_jsonl_sample(&fixture("sample.jsonl"), 2).expect("fixture should be readable");
        assert_eq!(sample.rows.len(), 2);
    }
}
