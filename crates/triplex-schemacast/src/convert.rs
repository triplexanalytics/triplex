//! The "zero-copy format converter" half of SchemaCast: reads a whole file
//! in one supported format and writes it back out in another. Output
//! formats it can currently write: `.csv`, `.tsv`, `.jsonl`, `.parquet`.
//! Arrow IPC/Feather and DuckDB table output aren't implemented — no writer
//! exists yet for either. Nothing here streams: both the read and the write
//! side materialize the whole file in memory, the same tradeoff the rest of
//! this crate already makes.

use crate::{infer_schema, unsupported_extension_error, ColumnType};
use serde::Serialize;
use std::path::Path;
use triplex_core::{Error, Result};
use triplex_io::{Sample, WriteColumnType};

#[derive(Debug, Serialize)]
pub struct ConvertReport {
    pub rows_converted: usize,
    pub input_format: String,
    pub output_format: String,
}

pub(crate) fn read_full(path: &Path, extension: &str, delimiter: Option<u8>) -> Result<Sample> {
    match (extension, delimiter) {
        ("csv", Some(d)) => triplex_io::read_csv_full_with_delimiter(path, d),
        ("parquet", _) => triplex_io::read_parquet_rows(path),
        _ => {
            let format = triplex_io::sample_format_for_extension(extension)
                .ok_or_else(|| unsupported_extension_error(path))?;
            format.read_full(path)
        }
    }
}

/// Byte-slice counterpart to `read_full`, for a caller with no filesystem
/// (`apps/triplex-web`'s browser/WASM path) — also the full-file grid-display
/// read for that same browser UI, not just schema inference's internal use.
/// No `.parquet` support here — see `crates/triplex-io/Cargo.toml` for why
/// Parquet input is out of scope for the wasm32 target in the first place.
pub fn read_full_bytes(bytes: &[u8], extension: &str) -> Result<Sample> {
    let format = triplex_io::sample_format_for_extension(extension).ok_or_else(|| {
        Error::UnsupportedExtension(format!(
            "unsupported file extension \"{extension}\" (expected .csv, .tsv, or .jsonl — \
             .parquet input isn't supported in the browser)"
        ))
    })?;
    format.read_full_bytes(bytes)
}

pub(crate) fn to_write_column_type(column_type: ColumnType) -> WriteColumnType {
    match column_type {
        ColumnType::Integer => WriteColumnType::Integer,
        ColumnType::Float => WriteColumnType::Float,
        ColumnType::Boolean => WriteColumnType::Boolean,
        ColumnType::String => WriteColumnType::String,
    }
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string()
}

/// Converts `input_path` to `output_path`, dispatching both sides on file
/// extension. Writing `.parquet` first runs the same type inference
/// `triplex schema` uses, so the output is properly typed rather than an
/// all-string dump; every other output format keeps the input's string
/// values as-is.
pub fn convert_file(input_path: &Path, output_path: &Path) -> Result<ConvertReport> {
    convert_file_with_delimiters(input_path, output_path, None, None)
}

/// Like `convert_file`, but overrides the field separator used for reading
/// a `.csv` input and/or writing a `.csv` output — e.g. converting a
/// semicolon-delimited European export to a pipe-delimited output. Each
/// delimiter is ignored unless that side's extension is actually `.csv`:
/// TSV's separator is fixed at tab, and JSONL/Parquet have no delimiter
/// concept.
pub fn convert_file_with_delimiters(
    input_path: &Path,
    output_path: &Path,
    input_delimiter: Option<u8>,
    output_delimiter: Option<u8>,
) -> Result<ConvertReport> {
    let input_format = extension_of(input_path);
    let output_format = extension_of(output_path);

    let sample = read_full(input_path, &input_format, input_delimiter)?;

    match output_format.as_str() {
        "csv" => match output_delimiter {
            Some(d) => triplex_io::write_csv_with_delimiter(output_path, d, &sample.headers, &sample.rows)?,
            None => triplex_io::write_csv(output_path, &sample.headers, &sample.rows)?,
        },
        "tsv" => triplex_io::write_tsv(output_path, &sample.headers, &sample.rows)?,
        "jsonl" => triplex_io::write_jsonl(output_path, &sample.headers, &sample.rows)?,
        "parquet" => {
            let schema = infer_schema(&sample);
            let column_types: Vec<WriteColumnType> = schema
                .columns
                .iter()
                .map(|c| to_write_column_type(c.inferred_type))
                .collect();
            triplex_io::write_parquet(output_path, &sample.headers, &column_types, &sample.rows)?;
        }
        _ => return Err(unsupported_extension_error(output_path)),
    }

    Ok(ConvertReport {
        rows_converted: sample.rows.len(),
        input_format,
        output_format,
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

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "triplex_schemacast_convert_test_{}_{name}",
            std::process::id()
        ))
    }

    #[test]
    fn read_full_bytes_matches_the_path_based_reader() {
        let bytes = std::fs::read(fixture("sample.csv")).unwrap();
        let from_bytes = read_full_bytes(&bytes, "csv").expect("bytes should parse");
        let from_path = read_full(&fixture("sample.csv"), "csv", None).unwrap();
        assert_eq!(from_bytes.headers, from_path.headers);
        assert_eq!(from_bytes.rows, from_path.rows);
    }

    #[test]
    fn read_full_bytes_rejects_parquet_with_a_clear_error_not_a_panic() {
        let bytes = std::fs::read(fixture("sample.parquet")).unwrap();
        let result = read_full_bytes(&bytes, "parquet");
        assert!(result.is_err(), "parquet input isn't supported via bytes");
    }

    #[test]
    fn converts_csv_to_tsv() {
        let output = temp_path("out.tsv");
        let report =
            convert_file(&fixture("sample.csv"), &output).expect("conversion should succeed");
        assert_eq!(report.input_format, "csv");
        assert_eq!(report.output_format, "tsv");
        assert_eq!(report.rows_converted, 4);

        let read_back = triplex_io::read_tsv_full(&output).expect("output should be readable");
        std::fs::remove_file(&output).ok();
        assert_eq!(
            read_back.headers,
            vec!["id", "price", "in_stock", "name"]
        );
        assert_eq!(read_back.rows[0], vec!["1", "9.99", "true", "widget"]);
    }

    #[test]
    fn converts_csv_to_jsonl() {
        let output = temp_path("out.jsonl");
        convert_file(&fixture("sample.csv"), &output).expect("conversion should succeed");
        let read_back = triplex_io::read_jsonl_full(&output).expect("output should be readable");
        std::fs::remove_file(&output).ok();
        assert_eq!(read_back.rows[0], vec!["1", "9.99", "true", "widget"]);
    }

    #[test]
    fn converts_csv_to_parquet_with_inferred_types() {
        let output = temp_path("out.parquet");
        convert_file(&fixture("sample.csv"), &output).expect("conversion should succeed");

        // Read back via schema inference to confirm the written types are
        // real Parquet types, not just strings that happen to look numeric.
        let contract = crate::infer_schema_from_path(&output, 100)
            .expect("parquet output should be readable");
        std::fs::remove_file(&output).ok();

        assert_eq!(contract.columns[0].inferred_type, ColumnType::Integer);
        assert_eq!(contract.columns[1].inferred_type, ColumnType::Float);
        assert_eq!(contract.columns[2].inferred_type, ColumnType::Boolean);
        assert_eq!(contract.columns[3].inferred_type, ColumnType::String);
    }

    #[test]
    fn converts_parquet_to_csv() {
        let output = temp_path("out_from_parquet.csv");
        let report = convert_file(&fixture("sample.parquet"), &output)
            .expect("conversion should succeed");
        assert_eq!(report.input_format, "parquet");
        assert_eq!(report.rows_converted, 4);

        let read_back = triplex_io::read_csv_full(&output).expect("output should be readable");
        std::fs::remove_file(&output).ok();
        assert_eq!(read_back.rows[0], vec!["1", "9.99", "true", "widget"]);
    }

    /// Every format pair, both directions, including identity conversions —
    /// proves Integer/Boolean/String values (including a zero-padded code
    /// like a zip, which only survives because of the leading-zero fix in
    /// `infer_column_type`) come out byte-for-byte identical no matter which
    /// format the data passes through, including Parquet.
    #[test]
    fn every_format_pair_preserves_integer_boolean_string_values_exactly() {
        let formats = ["csv", "tsv", "jsonl", "parquet"];
        let expected_headers = vec![
            "id".to_string(),
            "zip".to_string(),
            "active".to_string(),
            "label".to_string(),
        ];
        let expected_rows = vec![
            vec!["1".into(), "007".into(), "true".into(), "widget".into()],
            vec!["2".into(), "012".into(), "false".into(), "gadget".into()],
            vec!["3".into(), "099".into(), "true".into(), "sprocket".into()],
        ];

        let canonical = temp_path("matrix_canonical.csv");
        triplex_io::write_csv(&canonical, &expected_headers, &expected_rows).unwrap();

        // One "hub" file per format, all independently derived from the same
        // canonical source, so every format is a real conversion target and
        // source, not just re-reading the original CSV under a new name.
        let hubs: std::collections::HashMap<&str, std::path::PathBuf> = formats
            .iter()
            .map(|&fmt| {
                let hub = temp_path(&format!("matrix_hub.{fmt}"));
                if fmt == "csv" {
                    std::fs::copy(&canonical, &hub).unwrap();
                } else {
                    convert_file(&canonical, &hub)
                        .unwrap_or_else(|e| panic!("csv -> {fmt} hub conversion failed: {e}"));
                }
                (fmt, hub)
            })
            .collect();

        for &from in &formats {
            for &to in &formats {
                let output = temp_path(&format!("matrix_{from}_to_{to}.{to}"));
                convert_file(&hubs[from], &output)
                    .unwrap_or_else(|e| panic!("{from} -> {to} should succeed: {e}"));

                let actual = read_full(&output, to, None)
                    .unwrap_or_else(|e| panic!("{from} -> {to} output should be readable: {e}"));
                assert_eq!(
                    actual.headers, expected_headers,
                    "{from} -> {to} changed the header order/names"
                );
                assert_eq!(
                    actual.rows, expected_rows,
                    "{from} -> {to} did not preserve values exactly"
                );

                std::fs::remove_file(&output).ok();
            }
        }

        std::fs::remove_file(&canonical).ok();
        for hub in hubs.values() {
            std::fs::remove_file(hub).ok();
        }
    }

    /// Floats are the one value domain that can't make the same "100%
    /// intact in every direction" claim. Text-only hops (no Parquet) keep
    /// the exact source formatting; any hop through Parquet normalizes it
    /// (binary floats don't store display formatting), which is expected
    /// and pinned here, not silently accepted as a surprise.
    #[test]
    fn float_formatting_survives_text_only_hops_but_is_normalized_through_parquet() {
        let headers = vec!["price".to_string()];
        let original_rows = vec![vec!["3.50".to_string()], vec!["12.00".to_string()]];

        let csv_path = temp_path("float_original.csv");
        triplex_io::write_csv(&csv_path, &headers, &original_rows).unwrap();

        let tsv_path = temp_path("float.tsv");
        let jsonl_path = temp_path("float.jsonl");
        let back_via_text = temp_path("float_back_via_text.csv");
        convert_file(&csv_path, &tsv_path).unwrap();
        convert_file(&tsv_path, &jsonl_path).unwrap();
        convert_file(&jsonl_path, &back_via_text).unwrap();
        let text_only_result = triplex_io::read_csv_full(&back_via_text).unwrap();
        assert_eq!(
            text_only_result.rows, original_rows,
            "CSV -> TSV -> JSONL -> CSV must preserve exact float text, no native type involved"
        );

        let parquet_path = temp_path("float.parquet");
        let back_via_parquet = temp_path("float_back_via_parquet.csv");
        convert_file(&csv_path, &parquet_path).unwrap();
        convert_file(&parquet_path, &back_via_parquet).unwrap();
        let parquet_result = triplex_io::read_csv_full(&back_via_parquet).unwrap();
        assert_eq!(
            parquet_result.rows,
            vec![vec!["3.5".to_string()], vec!["12.0".to_string()]],
            "a Parquet hop normalizes float text (trailing zeros dropped) — numerically \
             identical to the source, not textually identical, and that's expected"
        );

        for p in [
            csv_path,
            tsv_path,
            jsonl_path,
            back_via_text,
            parquet_path,
            back_via_parquet,
        ] {
            std::fs::remove_file(&p).ok();
        }
    }

    #[test]
    fn converts_a_semicolon_delimited_csv_export_to_pipe_delimited_output() {
        let output = temp_path("out_pipe.csv");
        let report = convert_file_with_delimiters(
            &fixture("sample.semicolon.csv"),
            &output,
            Some(b';'),
            Some(b'|'),
        )
        .expect("conversion should succeed");
        assert_eq!(report.rows_converted, 3);

        let read_back = triplex_io::read_csv_full_with_delimiter(&output, b'|')
            .expect("output should be readable with the pipe delimiter");
        std::fs::remove_file(&output).ok();
        assert_eq!(
            read_back.headers,
            vec!["id", "price", "in_stock", "name"]
        );
        assert_eq!(read_back.rows[0], vec!["1", "9.99", "true", "widget"]);
    }

    #[test]
    fn an_unsupported_output_extension_is_a_clear_error() {
        let result = convert_file(&fixture("sample.csv"), Path::new("out.xlsx"));
        assert!(result.is_err());
    }

    #[test]
    fn an_unsupported_input_extension_is_a_clear_error() {
        let result = convert_file(Path::new("in.xlsx"), &temp_path("out.csv"));
        assert!(result.is_err());
    }
}
