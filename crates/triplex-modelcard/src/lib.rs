//! ModelCard: predictions-first fairness and slice metrics for a binary
//! classifier. The evaluation file carries the true label, the model's prediction and an
//! optional score; this crate never loads a model. It reads every row (metrics over a sample
//! would be wrong), reports per-slice metrics, checks group fairness against thresholds, and
//! emits a deterministic `model_card.json`, an offline `report.html` and a `reproduce.py`.
//!
//! The card is a documentation aid. It makes no compliance claim (see `DISCLAIMER`).

mod fairness;
mod metrics;
mod report_html;
mod report_script;

pub use fairness::{Fairness, GroupRates};
pub use metrics::{Metrics, Row};
pub use report_html::render_html;
pub use report_script::render_python_script;

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use triplex_core::{Error, Result};
use triplex_io::Sample;
use triplex_schemacast::{DeclaredContract, DeclaredModelCard};

pub const REPORT_VERSION: &str = "0.1.0";
pub const DEFAULT_MIN_SLICE_SIZE: usize = 30;
pub const DEFAULT_MAX_GROUPS_PER_COLUMN: usize = 50;
pub const DEFAULT_DISPARATE_IMPACT_MIN: f64 = 0.80;
pub const DEFAULT_EQUALIZED_ODDS_MIN: f64 = 0.85;
const NULL_GROUP: &str = "(null)";

/// Shown on every card. Deliberately avoids claiming that any regulation is met.
pub const DISCLAIMER: &str = "This card is a documentation aid. It is not a conformity \
assessment, legal advice, or a statement that any regulation or standard is satisfied.";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupResult {
    pub group: String,
    pub n: usize,
    /// Below the minimum slice size: only `n` is reported, no derived metric.
    pub suppressed: bool,
    pub metrics: Option<Metrics>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SliceColumn {
    pub column: String,
    pub groups: Vec<GroupResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    Min,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    Pass,
    Fail,
    /// The metric could not be computed (fewer than two comparable groups).
    Undefined,
    /// No threshold configured: reported, not gated.
    Informational,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FairnessCheck {
    pub name: String,
    pub kind: CheckKind,
    pub value: Option<f64>,
    pub threshold: Option<f64>,
    pub outcome: CheckOutcome,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FairnessReport {
    pub protected_attribute: String,
    pub groups_compared: Vec<String>,
    /// Groups left out because they were below the minimum slice size.
    pub groups_suppressed: Vec<String>,
    pub metrics: Fairness,
    pub checks: Vec<FairnessCheck>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    /// Process exit code: 0 pass, 1 warning, 2 a fairness threshold breach.
    pub fn exit_code(self) -> u8 {
        match self {
            Status::Pass => 0,
            Status::Warn => 1,
            Status::Fail => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    pub framework: Option<String>,
    pub intended_use: Option<String>,
    pub training_data: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DataSummary {
    pub rows_read: usize,
    pub rows_used: usize,
    /// Rows dropped because the true label or the prediction was empty.
    pub rows_dropped_missing_label: usize,
    pub positive_label: String,
    pub min_slice_size: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComplianceSection {
    pub heading: &'static str,
    /// Frameworks whose documentation this section is structured to line up with.
    pub aligned_with: &'static [&'static str],
    pub covers: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DocumentationAid {
    pub disclaimer: &'static str,
    pub sections: Vec<ComplianceSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelCardReport {
    pub version: &'static str,
    pub status: Status,
    pub model: ModelMetadata,
    pub data: DataSummary,
    pub overall: Metrics,
    pub slices: Vec<SliceColumn>,
    pub fairness: Option<FairnessReport>,
    pub warnings: Vec<String>,
    pub documentation_aid: DocumentationAid,
}

fn documentation_aid() -> DocumentationAid {
    DocumentationAid {
        disclaimer: DISCLAIMER,
        sections: vec![
            ComplianceSection {
                heading: "Intended use and limitations",
                aligned_with: &["EU AI Act Annex IV: general description, capabilities and limitations", "NIST AI RMF: Map"],
                covers: "model metadata and intended use as declared in the config",
            },
            ComplianceSection {
                heading: "Data and slice coverage",
                aligned_with: &["EU AI Act Annex IV: data used for evaluation", "NIST AI RMF: Map"],
                covers: "rows used, dropped rows, slice sizes and suppressed slices",
            },
            ComplianceSection {
                heading: "Performance, including for groups",
                aligned_with: &["EU AI Act Annex IV: accuracy for specific groups of persons", "NIST AI RMF: Measure"],
                covers: "overall and per-slice accuracy, precision, recall, false-positive rate, F1, ROC-AUC",
            },
            ComplianceSection {
                heading: "Fairness evaluation",
                aligned_with: &["NIST AI RMF: Measure"],
                covers: "group fairness metrics across the protected attribute and their thresholds",
            },
        ],
    }
}

fn normalize_label(v: &str) -> String {
    v.trim().to_lowercase()
}

fn read_rows(path: &Path) -> Result<Sample> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if extension == "parquet" {
        return triplex_io::read_parquet_rows(path);
    }
    match triplex_io::sample_format_for_extension(extension) {
        Some(format) => format.read_full(path),
        None => Err(Error::UnsupportedExtension(format!(
            "unsupported file extension \"{extension}\" (expected .csv, .tsv, .jsonl, or .parquet)"
        ))),
    }
}

fn column_index(sample: &Sample, name: &str) -> Result<usize> {
    sample
        .headers
        .iter()
        .position(|h| h == name)
        .ok_or_else(|| Error::ColumnNotFound(name.to_string()))
}

fn evaluate_check(
    name: &str,
    kind: CheckKind,
    value: Option<f64>,
    threshold: Option<f64>,
) -> FairnessCheck {
    let outcome = match (value, threshold) {
        (None, _) => CheckOutcome::Undefined,
        (Some(_), None) => CheckOutcome::Informational,
        // `!(a >= b)`, not `a < b`: a NaN threshold must fail closed, as in SchemaCast's
        // contract checks.
        (Some(v), Some(t)) => {
            let ok = match kind {
                CheckKind::Min => v >= t,
                CheckKind::Max => v <= t,
            };
            if ok {
                CheckOutcome::Pass
            } else {
                CheckOutcome::Fail
            }
        }
    };
    FairnessCheck {
        name: name.to_string(),
        kind,
        value,
        threshold,
        outcome,
    }
}

/// Evaluates an already-loaded evaluation table. See the crate docs for what is computed.
pub fn evaluate(sample: &Sample, config: &DeclaredModelCard) -> Result<ModelCardReport> {
    let min_slice_size = config.min_slice_size.unwrap_or(DEFAULT_MIN_SLICE_SIZE);
    let max_groups = config
        .max_groups_per_column
        .unwrap_or(DEFAULT_MAX_GROUPS_PER_COLUMN);
    let positive_label = config
        .positive_label
        .clone()
        .unwrap_or_else(|| "1".to_string());
    let positive = normalize_label(&positive_label);

    let target_i = column_index(sample, &config.target_column)?;
    let prediction_i = column_index(sample, &config.prediction_column)?;
    let score_i = config
        .score_column
        .as_deref()
        .map(|c| column_index(sample, c))
        .transpose()?;

    let mut slice_columns: Vec<String> = Vec::new();
    for name in config
        .slice_by
        .iter()
        .chain(config.protected_attribute.iter())
    {
        if !slice_columns.contains(name) {
            slice_columns.push(name.clone());
        }
    }
    let slice_indices: Vec<usize> = slice_columns
        .iter()
        .map(|c| column_index(sample, c))
        .collect::<Result<_>>()?;

    // Normalize labels; rows with an empty true label or prediction are dropped and counted.
    let mut kept: Vec<usize> = Vec::new();
    let mut labels: Vec<(String, String)> = Vec::new();
    for (row_number, row) in sample.rows.iter().enumerate() {
        let cell = |i: usize| row.get(i).map(String::as_str).unwrap_or("");
        let (actual, predicted) = (
            normalize_label(cell(target_i)),
            normalize_label(cell(prediction_i)),
        );
        if actual.is_empty() || predicted.is_empty() {
            continue;
        }
        kept.push(row_number);
        labels.push((actual, predicted));
    }
    if kept.is_empty() {
        return Err(Error::Validation(
            "no usable rows: every row has an empty label or prediction".into(),
        ));
    }
    let distinct: BTreeSet<&str> = labels
        .iter()
        .flat_map(|(a, p)| [a.as_str(), p.as_str()])
        .collect();
    if distinct.len() > 2 {
        return Err(Error::Validation(format!(
            "the label and prediction columns hold {} distinct values; ModelCard v1 supports binary classification only",
            distinct.len()
        )));
    }
    if distinct.len() == 2 && !distinct.contains(positive.as_str()) {
        return Err(Error::Validation(format!(
            "positive_label \"{positive_label}\" is not one of the two label values in the data"
        )));
    }

    let mut rows: Vec<Row> = Vec::with_capacity(kept.len());
    for (position, &row_number) in kept.iter().enumerate() {
        let score = match score_i {
            None => None,
            Some(i) => {
                let raw = sample.rows[row_number]
                    .get(i)
                    .map(String::as_str)
                    .unwrap_or("");
                // Plain notation only (dot decimal, no separators). Deliberately not
                // `normalize_numeric_string`: it reads "0.639" as a thousands-grouped 639, which
                // is right for accounting exports and wrong for a probability score.
                let parsed = raw.trim().parse::<f64>().ok().filter(|s| s.is_finite());
                // Report the data row number, never the offending value.
                Some(parsed.ok_or_else(|| {
                    Error::Validation(format!("the score column has an empty, non-numeric or non-finite value at data row {}", row_number + 1))
                })?)
            }
        };
        rows.push(Row {
            actual: labels[position].0 == positive,
            predicted: labels[position].1 == positive,
            score,
        });
    }

    let mut slices = Vec::new();
    let mut warnings = Vec::new();
    let mut rates_by_column: BTreeMap<String, Vec<(String, usize, Option<Metrics>)>> =
        BTreeMap::new();
    for (name, &column_i) in slice_columns.iter().zip(&slice_indices) {
        let mut members: BTreeMap<String, Vec<Row>> = BTreeMap::new();
        for (position, &row_number) in kept.iter().enumerate() {
            let raw = sample.rows[row_number]
                .get(column_i)
                .map(String::as_str)
                .unwrap_or("");
            let group = if raw.trim().is_empty() {
                NULL_GROUP.to_string()
            } else {
                raw.to_string()
            };
            members.entry(group).or_default().push(rows[position]);
        }
        if members.len() > max_groups {
            return Err(Error::Validation(format!(
                "slice column \"{name}\" has {} distinct values, more than the cap of {max_groups}; it looks like an identifier, not a slice",
                members.len()
            )));
        }
        let mut groups = Vec::new();
        let mut for_fairness = Vec::new();
        for (group, group_rows) in &members {
            let n = group_rows.len();
            if n < min_slice_size {
                warnings.push(format!("slice {name}={group} suppressed: n={n} is below the minimum slice size {min_slice_size}"));
                groups.push(GroupResult {
                    group: group.clone(),
                    n,
                    suppressed: true,
                    metrics: None,
                });
                for_fairness.push((group.clone(), n, None));
            } else {
                let m = metrics::compute(group_rows);
                for_fairness.push((group.clone(), n, Some(m.clone())));
                groups.push(GroupResult {
                    group: group.clone(),
                    n,
                    suppressed: false,
                    metrics: Some(m),
                });
            }
        }
        rates_by_column.insert(name.clone(), for_fairness);
        // Report only the columns the user asked to slice by, plus the protected attribute.
        slices.push(SliceColumn {
            column: name.clone(),
            groups,
        });
    }

    let dropped = sample.rows.len() - kept.len();
    if dropped > 0 {
        warnings.push(format!(
            "{dropped} row(s) dropped: empty true label or prediction"
        ));
    }

    let fairness = config.protected_attribute.as_ref().map(|attribute| {
        let entries = &rates_by_column[attribute];
        let compared: Vec<GroupRates> = entries
            .iter()
            .filter_map(|(group, _, m)| {
                m.as_ref().map(|m| GroupRates {
                    group: group.clone(),
                    selection_rate: m.selection_rate,
                    tpr: m.recall,
                    fpr: m.fpr,
                })
            })
            .collect();
        let suppressed: Vec<String> = entries
            .iter()
            .filter(|(_, _, m)| m.is_none())
            .map(|(g, _, _)| g.clone())
            .collect();
        let metrics = fairness::compute(&compared);
        let t = &config.thresholds;
        let checks = vec![
            evaluate_check(
                "disparate_impact_ratio",
                CheckKind::Min,
                metrics.disparate_impact_ratio,
                Some(
                    t.disparate_impact_min
                        .unwrap_or(DEFAULT_DISPARATE_IMPACT_MIN),
                ),
            ),
            evaluate_check(
                "equalized_odds_ratio",
                CheckKind::Min,
                metrics.equalized_odds_ratio,
                Some(t.equalized_odds_min.unwrap_or(DEFAULT_EQUALIZED_ODDS_MIN)),
            ),
            evaluate_check(
                "demographic_parity_difference",
                CheckKind::Max,
                metrics.demographic_parity_difference,
                t.demographic_parity_difference_max,
            ),
        ];
        FairnessReport {
            protected_attribute: attribute.clone(),
            groups_compared: compared.iter().map(|g| g.group.clone()).collect(),
            groups_suppressed: suppressed,
            metrics,
            checks,
        }
    });

    if let Some(report) = &fairness {
        if report.groups_compared.len() < 2 {
            warnings.push(format!(
                "protected attribute \"{}\" has fewer than two comparable groups; fairness metrics are undefined",
                report.protected_attribute
            ));
        }
    }

    let failed = fairness
        .as_ref()
        .is_some_and(|f| f.checks.iter().any(|c| c.outcome == CheckOutcome::Fail));
    let undefined = fairness.as_ref().is_some_and(|f| {
        f.checks
            .iter()
            .any(|c| c.outcome == CheckOutcome::Undefined)
    });
    let status = if failed {
        Status::Fail
    } else if undefined || !warnings.is_empty() {
        Status::Warn
    } else {
        Status::Pass
    };

    Ok(ModelCardReport {
        version: REPORT_VERSION,
        status,
        model: ModelMetadata {
            name: config.model.name.clone(),
            version: config.model.version.clone(),
            framework: config.model.framework.clone(),
            intended_use: config.model.intended_use.clone(),
            training_data: config.model.training_data.clone(),
        },
        data: DataSummary {
            rows_read: sample.rows.len(),
            rows_used: kept.len(),
            rows_dropped_missing_label: dropped,
            positive_label,
            min_slice_size,
        },
        overall: metrics::compute(&rows),
        slices,
        fairness,
        warnings,
        documentation_aid: documentation_aid(),
    })
}

/// Reads the whole evaluation file at `path` and evaluates it.
pub fn evaluate_path(path: &Path, config: &DeclaredModelCard) -> Result<ModelCardReport> {
    evaluate(&read_rows(path)?, config)
}

/// The `modelcard:` block of a loaded contract, or a clear error if there is none.
pub fn config_from_contract(contract: &DeclaredContract) -> Result<&DeclaredModelCard> {
    contract
        .modelcard
        .as_ref()
        .ok_or_else(|| Error::Validation("the contract has no `modelcard:` block".into()))
}
