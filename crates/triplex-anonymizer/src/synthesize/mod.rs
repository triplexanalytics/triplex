//! Tier 2 synthesis, slice T2.1: a seeded Gaussian-copula generator with a
//! fidelity report and a leakage report. It makes NO formal privacy claim (differential privacy
//! is slice T2.2); the report says so, and leakage is measured, not certified.
//!
//! Columns that look like identifiers are never synthesized (an identifier-grade semantic type
//! such as email or IBAN, a location-named column that is at least half unique, a string column
//! that is at least 90% unique, an integer column that is nearly all distinct, which SchemaCast flags
//! as a likely identifier): Tier 1 masking is the tool for those.
//! Categories seen fewer than `min_category_count` times are merged into `(other)` before fitting,
//! so a singleton category (one real person) is never reproduced.
//!
//! An integer column with 20 or fewer distinct values (a code, a rating) is sampled from its
//! empirical distribution without interpolation, so it never invents a value that did not occur.

pub mod bayesnet;
pub mod dp;
pub mod sketch;

mod copula;
mod metrics;
mod numeric;
mod report_html;
mod report_script;

pub use copula::ColumnData;
pub use metrics::{ColumnFidelity, CorrelationFidelity, Fidelity, Leakage};
pub use report_html::render_html;
pub use report_script::render_python_script;

use copula::Copula;
use numeric::Rng;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use triplex_core::{normalize_numeric_string, Error, Result};
use triplex_io::Sample;
use triplex_schemacast::{infer_schema, ColumnType, SemanticType};

pub const REPORT_VERSION: &str = "0.1.0";
pub const OTHER_CATEGORY: &str = "(other)";
const MIN_ROWS: usize = 10;
/// A `Location`-named column with at least this share of unique values is treated as identifying.
const LOCATION_MAX_UNIQUE_RATIO: f64 = 0.5;

pub const PRIVACY_STATEMENT: &str = "Synthetic data with no formal privacy guarantee. Leakage \
is measured in this report, not certified; do not treat the output as anonymous.";

#[derive(Debug, Clone)]
pub struct SynthesizeConfig {
    /// Rows to generate; `None` means as many as the input has.
    pub rows: Option<usize>,
    pub seed: u64,
    pub min_category_count: usize,
    /// Columns the user removed from the output: never synthesized, listed as dropped.
    pub exclude_columns: Vec<String>,
    /// 0-based input rows the user removed: dropped before anything is fitted or measured.
    pub exclude_rows: std::collections::HashSet<usize>,
}

impl Default for SynthesizeConfig {
    fn default() -> Self {
        SynthesizeConfig {
            rows: None,
            seed: 42,
            min_category_count: 5,
            exclude_columns: Vec::new(),
            exclude_rows: std::collections::HashSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DroppedColumn {
    pub column: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergedCategories {
    pub column: String,
    pub categories_merged: usize,
    pub rows_affected: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SynthesisReport {
    pub version: &'static str,
    pub method: &'static str,
    pub seed: u64,
    pub rows_in: usize,
    /// Input rows removed before fitting (`SynthesizeConfig::exclude_rows`).
    pub rows_excluded: usize,
    pub rows_out: usize,
    pub columns_synthesized: Vec<String>,
    pub columns_dropped: Vec<DroppedColumn>,
    pub categories_merged: Vec<MergedCategories>,
    /// 0 when the fitted correlation matrix needed no repair.
    pub correlation_shrinkage: f64,
    pub fidelity: Fidelity,
    pub leakage: Leakage,
    pub privacy_statement: &'static str,
    pub limitations: Vec<&'static str>,
    pub warnings: Vec<String>,
    /// The dependency structure, for the Bayesian-network methods.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<bayesnet::NetworkReport>,
    /// The classes of the sketch, for the sketch method (Tier 3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sketch: Option<sketch::SketchSummary>,
}

pub struct SynthesisResult {
    pub sample: Sample,
    pub report: SynthesisReport,
}

fn limitations() -> Vec<&'static str> {
    vec![
        "Nulls are modelled per column and independently of the other columns.",
        "Categorical columns enter the copula through their cumulative-frequency mid-point, which attenuates correlations involving them.",
        "Only numeric-pair correlations are measured; no multi-column or conditional structure is checked.",
        "Identifier-like columns and dates are dropped, not synthesized.",
    ]
}

pub(crate) struct Prepared {
    pub(crate) name: String,
    /// Original real values (categories not merged), used for leakage.
    pub(crate) original: ColumnData,
    /// Real values after rare-category merging: what the model is fitted on.
    pub(crate) merged: ColumnData,
    pub(crate) integer: bool,
    /// Decimal places seen in the real strings of a float column; synthetic values are rounded to
    /// this many so they do not carry precision the real data never had.
    pub(crate) decimals: Option<usize>,
}

fn cell(sample: &Sample, row: usize, column: usize) -> Option<&str> {
    let v = sample.rows[row].get(column).map(|s| s.trim()).unwrap_or("");
    (!v.is_empty()).then_some(v)
}

fn format_number(x: f64, integer: bool, decimals: Option<usize>) -> String {
    match (integer, decimals) {
        (true, _) => format!("{}", x as i64),
        (false, Some(d)) => format!("{x:.d$}"),
        (false, None) => format!("{x}"),
    }
}

/// Most decimal places in the normalized numeric strings; `None` for scientific notation or
/// anything above 10 places, where rounding would change the data's character.
fn observed_decimals(values: &[&str]) -> Option<usize> {
    let mut most = 0;
    for v in values {
        let normalized = normalize_numeric_string(v);
        if normalized.contains(['e', 'E']) {
            return None;
        }
        if let Some((_, fraction)) = normalized.split_once('.') {
            most = most.max(fraction.len());
        }
    }
    (most <= 10).then_some(most)
}

/// A copy of `sample` without the given 0-based rows, and how many rows were actually removed
/// (an out-of-range index removes nothing).
pub(crate) fn without_rows(
    sample: &Sample,
    excluded: &std::collections::HashSet<usize>,
) -> (Sample, usize) {
    let rows: Vec<Vec<String>> = sample
        .rows
        .iter()
        .enumerate()
        .filter(|(i, _)| !excluded.contains(i))
        .map(|(_, row)| row.clone())
        .collect();
    let removed = sample.rows.len() - rows.len();
    (
        Sample {
            headers: sample.headers.clone(),
            rows,
        },
        removed,
    )
}

/// The columns a synthesizer can model, after dropping identifiers, merging rare categories and
/// removing the rows and columns the user excluded. Shared by every non-private model.
pub(crate) struct Preparation {
    pub(crate) prepared: Vec<Prepared>,
    pub(crate) dropped: Vec<DroppedColumn>,
    pub(crate) merged_report: Vec<MergedCategories>,
    /// Rows left after the exclusion.
    pub(crate) n: usize,
    pub(crate) rows_excluded: usize,
}

pub(crate) fn prepare(sample: &Sample, config: &SynthesizeConfig) -> Result<Preparation> {
    let filtered;
    let (sample, rows_excluded) = if config.exclude_rows.is_empty() {
        (sample, 0)
    } else {
        let (kept, removed) = without_rows(sample, &config.exclude_rows);
        filtered = kept;
        (&filtered, removed)
    };
    let n = sample.rows.len();
    if n < MIN_ROWS {
        return Err(Error::Validation(format!(
            "need at least {MIN_ROWS} rows to fit a distribution, got {n}"
        )));
    }
    let schema = infer_schema(sample);
    let mut prepared: Vec<Prepared> = Vec::new();
    let mut dropped = Vec::new();
    let mut merged_report = Vec::new();

    for (index, column) in schema.columns.iter().enumerate() {
        let raw: Vec<Option<&str>> = (0..n).map(|r| cell(sample, r, index)).collect();
        let non_null: Vec<&str> = raw.iter().flatten().copied().collect();
        let mut drop = |reason: String| {
            dropped.push(DroppedColumn {
                column: column.name.clone(),
                reason,
            })
        };
        if config.exclude_columns.contains(&column.name) {
            drop("removed from the output by the user".into());
            continue;
        }
        if non_null.is_empty() {
            drop("every value is empty".into());
            continue;
        }
        let distinct: HashSet<&str> = non_null.iter().copied().collect();
        let unique_ratio = distinct.len() as f64 / non_null.len() as f64;
        if let Some(semantic) = column.semantic_type {
            // `Location` is only a hint from the column *name* ("region", "city"): a low-cardinality
            // one is a plain category, and rare-category merging already protects its singletons.
            // Every other semantic type is identifier-grade.
            let coarse_location =
                semantic == SemanticType::Location && unique_ratio < LOCATION_MAX_UNIQUE_RATIO;
            if !coarse_location {
                drop(format!("detected as {semantic:?}, a sensitive identifier type; use Tier 1 masking instead"));
                continue;
            }
        }
        let integer = column.inferred_type == ColumnType::Integer;
        match column.inferred_type {
            ColumnType::Integer | ColumnType::Float => {
                let values: Vec<Option<f64>> = raw
                    .iter()
                    .map(|v| {
                        v.map(|v| normalize_numeric_string(v).parse::<f64>())
                            .transpose()
                    })
                    .collect::<std::result::Result<_, _>>()
                    .map_err(|_| {
                        Error::Validation(format!(
                            "column \"{}\" has a value that does not parse as a number",
                            column.name
                        ))
                    })?;
                let data = ColumnData::Numeric(values);
                let decimals = if integer {
                    None
                } else {
                    observed_decimals(&non_null)
                };
                prepared.push(Prepared {
                    name: column.name.clone(),
                    original: data.clone(),
                    merged: data,
                    integer,
                    decimals,
                });
            }
            ColumnType::Boolean | ColumnType::String => {
                if column.inferred_type == ColumnType::String && n >= 20 && unique_ratio >= 0.9 {
                    drop(format!(
                        "{:.0}% of values are unique, treated as an identifier",
                        unique_ratio * 100.0
                    ));
                    continue;
                }
                let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
                for v in &non_null {
                    *counts.entry(v).or_insert(0) += 1;
                }
                let rare: HashSet<&str> = counts
                    .iter()
                    .filter(|(_, &c)| c < config.min_category_count)
                    .map(|(k, _)| *k)
                    .collect();
                let original: Vec<Option<String>> =
                    raw.iter().map(|v| v.map(str::to_string)).collect();
                let merged: Vec<Option<String>> = original
                    .iter()
                    .map(|v| {
                        v.as_ref().map(|s| {
                            if rare.contains(s.as_str()) {
                                OTHER_CATEGORY.to_string()
                            } else {
                                s.clone()
                            }
                        })
                    })
                    .collect();
                if !rare.is_empty() {
                    merged_report.push(MergedCategories {
                        column: column.name.clone(),
                        categories_merged: rare.len(),
                        rows_affected: rare.iter().map(|k| counts[k]).sum(),
                    });
                }
                prepared.push(Prepared {
                    name: column.name.clone(),
                    original: ColumnData::Categorical(original),
                    merged: ColumnData::Categorical(merged),
                    integer: false,
                    decimals: None,
                });
            }
        }
    }
    if prepared.is_empty() {
        return Err(Error::Validation(
            "no column can be synthesized: every column is empty or looks like an identifier"
                .into(),
        ));
    }
    Ok(Preparation {
        prepared,
        dropped,
        merged_report,
        n,
        rows_excluded,
    })
}

/// Rounds each float column to the decimals its real strings had, through the written text, so
/// the metrics see exactly the values the file contains.
pub(crate) fn round_to_observed(synthetic: &mut [ColumnData], prepared: &[Prepared]) {
    for (column, p) in synthetic.iter_mut().zip(prepared) {
        if let (ColumnData::Numeric(values), Some(d)) = (column, p.decimals) {
            for v in values.iter_mut().flatten() {
                *v = format!("{v:.d$}").parse().unwrap_or(*v);
            }
        }
    }
}

/// The synthetic columns as written text, row by row.
pub(crate) fn format_rows(
    synthetic: &[ColumnData],
    prepared: &[Prepared],
    rows_out: usize,
) -> Vec<Vec<String>> {
    (0..rows_out)
        .map(|r| {
            synthetic
                .iter()
                .zip(prepared)
                .map(|(c, p)| match c {
                    ColumnData::Numeric(v) => {
                        v[r].map_or_else(String::new, |x| format_number(x, p.integer, p.decimals))
                    }
                    ColumnData::Categorical(v) => v[r].clone().unwrap_or_default(),
                })
                .collect()
        })
        .collect()
}

/// One warning per column that was not synthesized.
pub(crate) fn dropped_warnings(dropped: &[DroppedColumn]) -> Vec<String> {
    dropped
        .iter()
        .map(|d| format!("column \"{}\" not synthesized: {}", d.column, d.reason))
        .collect()
}

/// Synthesizes a table shaped like `sample`. See the module docs for what is and is not kept.
pub fn synthesize(sample: &Sample, config: &SynthesizeConfig) -> Result<SynthesisResult> {
    let Preparation {
        prepared,
        dropped,
        merged_report,
        n,
        rows_excluded,
    } = prepare(sample, config)?;
    let merged: Vec<ColumnData> = prepared.iter().map(|p| p.merged.clone()).collect();
    let originals: Vec<ColumnData> = prepared.iter().map(|p| p.original.clone()).collect();
    let integer_flags: Vec<bool> = prepared.iter().map(|p| p.integer).collect();
    let names: Vec<String> = prepared.iter().map(|p| p.name.clone()).collect();

    let copula = Copula::fit(&merged, &integer_flags);
    let rows_out = config.rows.unwrap_or(n);
    let mut synthetic = copula.sample(rows_out, &mut Rng::new(config.seed));
    round_to_observed(&mut synthetic, &prepared);
    let rows = format_rows(&synthetic, &prepared, rows_out);
    let mut warnings = dropped_warnings(&dropped);
    if copula.shrinkage > 0.0 {
        warnings.push(format!("the correlation matrix was not positive definite and was shrunk toward the identity by {:.2}", copula.shrinkage));
    }
    let report = SynthesisReport {
        version: REPORT_VERSION,
        method: "gaussian_copula",
        seed: config.seed,
        rows_in: n,
        rows_excluded,
        rows_out,
        columns_synthesized: names.clone(),
        columns_dropped: dropped,
        categories_merged: merged_report,
        correlation_shrinkage: copula.shrinkage,
        fidelity: metrics::fidelity(&names, &merged, &synthetic),
        leakage: metrics::leakage(&originals, &synthetic),
        privacy_statement: PRIVACY_STATEMENT,
        limitations: limitations(),
        warnings,
        network: None,
        sketch: None,
    };
    Ok(SynthesisResult {
        sample: Sample {
            headers: names,
            rows,
        },
        report,
    })
}

/// Reads `input_path` (`.csv` or `.parquet`), synthesizes, writes the result to `output_path` as
/// CSV, and returns the report.
pub fn synthesize_file(
    input_path: &Path,
    output_path: &Path,
    config: &SynthesizeConfig,
) -> Result<SynthesisReport> {
    let sample = crate::read_rows(input_path)?;
    let result = synthesize(&sample, config)?;
    triplex_io::write_csv(output_path, &result.sample.headers, &result.sample.rows)?;
    Ok(result.report)
}

/// Byte-slice counterpart to [`synthesize_file`], for a caller with no filesystem (the browser):
/// reads `.csv`, `.tsv` or `.jsonl` bytes, synthesizes, and returns the synthetic table as CSV
/// bytes together with the report.
pub fn synthesize_bytes(
    bytes: &[u8],
    extension: &str,
    config: &SynthesizeConfig,
) -> Result<(Vec<u8>, SynthesisReport)> {
    let sample = triplex_schemacast::read_full_bytes(bytes, extension)?;
    let result = synthesize(&sample, config)?;
    let csv = triplex_io::write_csv_bytes(&result.sample.headers, &result.sample.rows)?;
    Ok((csv, result.report))
}
