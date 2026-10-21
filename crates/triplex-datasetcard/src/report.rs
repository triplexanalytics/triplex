//! The dataset risk report: how well a table's quasi-identifiers hide each person, one column at
//! a time, in pairs, and all together, plus the rows most at risk.
//!
//! A quasi-identifier is a column that is harmless alone but that someone could know from
//! elsewhere: an age, a postcode, a plan. Each one alone may put every person in a large group,
//! while together they single people out. That is the finding this report exists for, so it
//! scores the combination, not only the columns.
//!
//! Numbers are generalized into `"lo-hi"` ranges before counting (an exact age singles out more
//! people than anyone needs to know), text and category values are compared exactly. The same
//! rules drive the web UI's privacy analysis, so both report the same numbers for the same table.

use serde::Serialize;
use triplex_core::{Error, Result};
use triplex_io::Sample;
use triplex_schemacast::{ColumnSchema, ColumnType, SchemaContract, SemanticType};

use crate::{
    bucket_numeric, bucket_value, compose_quasi_identifiers, dis_suda_scores, k_anonymity,
    k_anonymity_worst_group, row_risk_scores, RowRiskModel,
};

/// The bucket width the web UI's privacy analysis uses for numbers.
pub const DEFAULT_BUCKET_WIDTH: u64 = 10;
/// How many of the riskiest rows the report lists by default.
pub const DEFAULT_TOP_ROWS: usize = 5;

/// What to assess, and how.
#[derive(Debug, Clone)]
pub struct RiskReportOptions {
    /// The quasi-identifier columns, in order. `None` lets the report choose them (see
    /// `default_quasi_identifiers`).
    pub quasi_identifiers: Option<Vec<String>>,
    /// Width of the `"lo-hi"` ranges numbers are generalized into.
    pub bucket_width: u64,
    /// How each row's disclosure risk is scored.
    pub row_model: RowRiskModel,
    /// How many of the riskiest rows to list.
    pub top_rows: usize,
}

impl Default for RiskReportOptions {
    fn default() -> Self {
        Self {
            quasi_identifiers: None,
            bucket_width: DEFAULT_BUCKET_WIDTH,
            row_model: RowRiskModel::Suda,
            top_rows: DEFAULT_TOP_ROWS,
        }
    }
}

/// One column's value, as the report names it (`"30-39"` for a generalized number).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnValue {
    pub column: String,
    pub value: String,
}

/// k-anonymity of one quasi-identifier on its own: the size of the smallest group of rows that
/// share its value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnK {
    pub column: String,
    pub k: usize,
}

/// k-anonymity of two quasi-identifiers together.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PairK {
    pub columns: [String; 2],
    pub k: usize,
}

/// k-anonymity of every quasi-identifier together: the dataset-level verdict.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CombinedK {
    pub columns: Vec<String>,
    pub k: usize,
    /// Rows no other row shares all quasi-identifier values with: each one singles a person out.
    pub rows_alone: usize,
    /// The values of the smallest group (the one that sets `k`).
    pub smallest_group: Vec<ColumnValue>,
}

/// One of the rows most at risk.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RowScore {
    /// 1-based position among the data rows (the header is not counted).
    pub row: usize,
    pub score: f64,
    /// This row's share of the whole table's risk; the shares of all rows add up to 1.
    pub share: f64,
    pub values: Vec<ColumnValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RowRisk {
    pub model: RowRiskModel,
    pub top: Vec<RowScore>,
}

/// A column the report left out of the quasi-identifiers it chose itself, and why.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExcludedColumn {
    pub column: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DatasetRiskReport {
    pub rows: usize,
    pub quasi_identifiers: Vec<String>,
    /// Whether the quasi-identifiers were named by the caller or chosen by the report.
    pub quasi_identifiers_chosen_by: QuasiIdentifierSource,
    /// Only filled when the report chose: the columns it left out, with the reason.
    pub excluded_columns: Vec<ExcludedColumn>,
    pub bucket_width: u64,
    pub columns: Vec<ColumnK>,
    pub pairs: Vec<PairK>,
    pub combined: CombinedK,
    pub row_risk: RowRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuasiIdentifierSource {
    Caller,
    Default,
}

/// Why a column is not a default quasi-identifier, or `None` when it is one.
///
/// A default quasi-identifier is a whole-number or text/category column, the same rule as the
/// web UI's privacy analysis. Columns detected as identifiers are left out: an ID or an email
/// address singles out everyone on its own, which would hide the finding this report is for.
/// Detected locations and dates of birth stay in, since those are classic quasi-identifiers.
/// Decimal columns are left out because exact amounts rarely repeat; name one explicitly to
/// include it.
fn exclusion_reason(column: &ColumnSchema) -> Option<String> {
    if let Some(semantic) = column.semantic_type {
        if !matches!(semantic, SemanticType::Location | SemanticType::DateOfBirth) {
            return Some(match semantic {
                SemanticType::LikelyIdentifier => {
                    "detected as an identifier: almost every value is distinct".to_string()
                }
                other => format!("detected as {other:?}, a direct identifier"),
            });
        }
    }
    match column.inferred_type {
        ColumnType::Integer | ColumnType::String | ColumnType::Boolean => None,
        ColumnType::Float => Some("a decimal column; name it explicitly to include it".to_string()),
    }
}

/// The quasi-identifiers the report chooses when the caller names none, with every column it
/// left out and why.
pub fn default_quasi_identifiers(schema: &SchemaContract) -> (Vec<String>, Vec<ExcludedColumn>) {
    let mut chosen = Vec::new();
    let mut excluded = Vec::new();
    for column in &schema.columns {
        match exclusion_reason(column) {
            None => chosen.push(column.name.clone()),
            Some(reason) => excluded.push(ExcludedColumn {
                column: column.name.clone(),
                reason,
            }),
        }
    }
    (chosen, excluded)
}

/// Builds the risk report for every row of `sample`, whose columns `schema` describes (from
/// `triplex_schemacast::infer_schema` on the same sample).
///
/// Errors when the table has no rows, when a named quasi-identifier is not a column, when there
/// is no quasi-identifier to assess, or when the row-risk model refuses the input (SUDA caps the
/// number of quasi-identifiers it can search).
pub fn assess(
    sample: &Sample,
    schema: &SchemaContract,
    options: &RiskReportOptions,
) -> Result<DatasetRiskReport> {
    if sample.rows.is_empty() {
        return Err(Error::Validation("the table has no rows to assess".into()));
    }
    let (quasi, source, excluded) = match &options.quasi_identifiers {
        Some(named) => (named.clone(), QuasiIdentifierSource::Caller, Vec::new()),
        None => {
            let (chosen, excluded) = default_quasi_identifiers(schema);
            (chosen, QuasiIdentifierSource::Default, excluded)
        }
    };
    if quasi.is_empty() {
        return Err(Error::Validation(
            "no quasi-identifier columns to assess; name them explicitly".into(),
        ));
    }

    // Each quasi-identifier's values, generalized: numbers into ranges, text as it is.
    let mut generalized: Vec<Vec<String>> = Vec::with_capacity(quasi.len());
    for name in &quasi {
        let index = sample
            .headers
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| Error::Validation(format!("no column named {name:?}")))?;
        let numeric = schema
            .columns
            .iter()
            .find(|c| &c.name == name)
            .is_some_and(|c| matches!(c.inferred_type, ColumnType::Integer | ColumnType::Float));
        generalized.push(
            sample
                .rows
                .iter()
                .map(|row| {
                    let raw = row.get(index).map(String::as_str).unwrap_or("");
                    if numeric {
                        generalize_number(options.bucket_width, raw)
                    } else {
                        raw.to_string()
                    }
                })
                .collect(),
        );
    }

    let columns = quasi
        .iter()
        .zip(&generalized)
        .map(|(name, values)| ColumnK {
            column: name.clone(),
            k: k_anonymity(values),
        })
        .collect();

    let mut pairs = Vec::new();
    for i in 0..quasi.len() {
        for j in i + 1..quasi.len() {
            let composite =
                compose_quasi_identifiers(&[generalized[i].clone(), generalized[j].clone()]);
            pairs.push(PairK {
                columns: [quasi[i].clone(), quasi[j].clone()],
                k: k_anonymity(&composite),
            });
        }
    }

    let composite = compose_quasi_identifiers(&generalized);
    let mut group_sizes = std::collections::HashMap::<&str, usize>::new();
    for key in &composite {
        *group_sizes.entry(key.as_str()).or_insert(0) += 1;
    }
    let combined = CombinedK {
        columns: quasi.clone(),
        k: k_anonymity(&composite),
        rows_alone: composite
            .iter()
            .filter(|key| group_sizes[key.as_str()] == 1)
            .count(),
        smallest_group: k_anonymity_worst_group(&composite)
            .map(|key| named_values(&quasi, key.split('\u{1f}')))
            .unwrap_or_default(),
    };

    let scores = row_risk_scores(options.row_model, &generalized)?;
    let shares = dis_suda_scores(&scores);
    let mut order: Vec<usize> = (0..scores.len()).collect();
    // Riskiest first; equal scores keep table order, so the report is deterministic.
    order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));
    let top = order
        .into_iter()
        .take(options.top_rows)
        .map(|i| RowScore {
            row: i + 1,
            score: scores[i],
            share: shares[i],
            values: named_values(&quasi, generalized.iter().map(|c| c[i].as_str())),
        })
        .collect();

    Ok(DatasetRiskReport {
        rows: sample.rows.len(),
        quasi_identifiers: quasi,
        quasi_identifiers_chosen_by: source,
        excluded_columns: excluded,
        bucket_width: options.bucket_width,
        columns,
        pairs,
        combined,
        row_risk: RowRisk {
            model: options.row_model,
            top,
        },
    })
}

/// A number generalized into its range: a whole number exactly as `bucket_value` does, a decimal
/// the way `bucket_numeric` does for the web UI's typed numbers. Anything that is not a number,
/// an empty cell included, passes through unchanged and forms its own group.
fn generalize_number(width: u64, raw: &str) -> String {
    let raw = raw.trim();
    if raw.parse::<i64>().is_ok() {
        return bucket_value(width, raw);
    }
    match raw.parse::<f64>() {
        Ok(n) if n.is_finite() => bucket_numeric(width, n),
        _ => raw.to_string(),
    }
}

fn named_values<'a>(names: &[String], values: impl Iterator<Item = &'a str>) -> Vec<ColumnValue> {
    names
        .iter()
        .zip(values)
        .map(|(column, value)| ColumnValue {
            column: column.clone(),
            value: value.to_string(),
        })
        .collect()
}
