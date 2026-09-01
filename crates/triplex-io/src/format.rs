//! Strategy pattern: each ingestion format implements `SampleFormat` the
//! same way, so a caller that just wants rows from a file — bounded (like
//! SchemaCast's type inference) or complete (like SchemaCast's `convert`) —
//! picks a strategy by file extension via `sample_format_for_extension`
//! instead of hardcoding a match arm per format.
//!
//! Candidate for the same treatment later: Anonymizer's `AnonymizeMethod`
//! is a single-variant match today (`TokenHmac`); once a second method
//! (e.g. generalization/bucketing) exists, it's the same shape of problem.

use crate::csv_source::CsvFormat;
use crate::jsonl_source::JsonlFormat;
use crate::tsv_source::TsvFormat;
use crate::Sample;
use std::path::Path;
use triplex_core::Result;

pub trait SampleFormat {
    /// Reads a header plus up to `max_rows` data rows.
    fn read_sample(&self, path: &Path, max_rows: usize) -> Result<Sample>;
    /// Reads a header plus every data row.
    fn read_full(&self, path: &Path) -> Result<Sample>;
}

/// Looks up the `SampleFormat` strategy for a file extension (without the
/// leading dot), e.g. `"csv"`. Parquet is deliberately not included — it
/// has its own zero-copy, footer-metadata schema path instead of a bounded
/// row-sample, so it isn't shaped like the other formats here.
pub fn sample_format_for_extension(extension: &str) -> Option<Box<dyn SampleFormat>> {
    match extension {
        "csv" => Some(Box::new(CsvFormat)),
        "tsv" => Some(Box::new(TsvFormat)),
        "jsonl" => Some(Box::new(JsonlFormat)),
        _ => None,
    }
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
    fn each_known_extension_resolves_to_a_working_strategy() {
        for (ext, file) in [
            ("csv", "sample.csv"),
            ("tsv", "sample.tsv"),
            ("jsonl", "sample.jsonl"),
        ] {
            let format = sample_format_for_extension(ext)
                .unwrap_or_else(|| panic!("expected a strategy for .{ext}"));
            let sample = format
                .read_sample(&fixture(file), 100)
                .unwrap_or_else(|_| panic!("expected {file} to be readable via .{ext} strategy"));
            assert_eq!(sample.headers, vec!["id", "price", "in_stock", "name"]);
        }
    }

    #[test]
    fn read_full_returns_every_row_unlike_read_sample() {
        let format = sample_format_for_extension("csv").unwrap();
        let full = format.read_full(&fixture("sample.csv")).unwrap();
        let capped = format.read_sample(&fixture("sample.csv"), 2).unwrap();
        assert_eq!(full.rows.len(), 4);
        assert_eq!(capped.rows.len(), 2);
    }

    #[test]
    fn an_unknown_extension_has_no_strategy() {
        assert!(sample_format_for_extension("xlsx").is_none());
        assert!(sample_format_for_extension("parquet").is_none());
    }
}
