//! Declarative `.triplex.yaml` schema contracts and validation of an inferred
//! schema against one. No coercion modes yet — a column either matches the
//! declared contract or it doesn't.

use crate::{ColumnType, SchemaContract};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use triplex_core::{Error, Result};
use triplex_io::Sample;

fn default_required() -> bool {
    true
}

/// Statistical thresholds a declared column may be checked against. Grouped
/// separately from identity/type (name, column_type, required) since these
/// are all optional, orthogonal checks against the inferred column's
/// `ColumnStatistics` — see `triplex_io::ColumnStatistics`.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DeclaredColumnStatistics {
    #[serde(default)]
    pub max_null_percentage: Option<f64>,
    #[serde(default)]
    pub min_type_consistency: Option<f64>,
    #[serde(default)]
    pub min_value: Option<f64>,
    #[serde(default)]
    pub max_value: Option<f64>,
    /// The mean must fall within `expected_mean` ± `mean_epsilon`.
    /// `mean_epsilon` defaults to 0.0 (exact match) if omitted.
    #[serde(default)]
    pub expected_mean: Option<f64>,
    #[serde(default)]
    pub mean_epsilon: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DeclaredColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: ColumnType,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default)]
    pub statistics: Option<DeclaredColumnStatistics>,
    /// A regex every non-empty value in this column must match. Checked
    /// separately from `statistics` (see `check_patterns`) since it needs
    /// every raw value, not just aggregate stats over a sample.
    #[serde(default)]
    pub pattern: Option<String>,
}

impl DeclaredColumn {
    /// Gets (or lazily creates) this column's statistics thresholds — handy
    /// for building a contract programmatically rather than from YAML.
    pub fn statistics_mut(&mut self) -> &mut DeclaredColumnStatistics {
        self.statistics
            .get_or_insert_with(DeclaredColumnStatistics::default)
    }
}

/// Optional DriftGuard threshold overrides, layered onto the same
/// `.triplex.yaml` contract a dataset already declares its schema in --
/// the design notes's "Configuration:
/// version-controlled `.triplex.yaml` only" rule means DriftGuard gets no
/// config format of its own. Unset fields fall back to
/// `triplex_driftguard::DriftThresholds::default()`; schemacast only parses
/// this block, it has no opinion on what the values mean.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DeclaredDriftGuardThresholds {
    #[serde(default)]
    pub max_null_percentage_delta: Option<f64>,
    #[serde(default)]
    pub max_mean_relative_delta: Option<f64>,
    #[serde(default)]
    pub max_type_consistency_delta: Option<f64>,
}

/// Optional fairness thresholds for `triplex-modelcard`. Ratio metrics take a minimum, difference
/// metrics a maximum. Unset fields fall back to that crate's defaults (or, for the parity
/// difference, to "report only"); schemacast only parses them.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DeclaredFairnessThresholds {
    #[serde(default)]
    pub disparate_impact_min: Option<f64>,
    #[serde(default)]
    pub equalized_odds_min: Option<f64>,
    #[serde(default)]
    pub demographic_parity_difference_max: Option<f64>,
}

/// Model metadata declared by the user. ModelCard never loads a model, so none of this is
/// inspected: it is carried into the card verbatim.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DeclaredModelMetadata {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub intended_use: Option<String>,
    #[serde(default)]
    pub training_data: Option<String>,
}

/// The `modelcard:` block of a `.triplex.yaml`: which columns of an
/// evaluation file hold the true label, the model's prediction and optional score, how to slice
/// it, and which attribute to check for fairness. Parsing only; `triplex-modelcard` owns the
/// defaults and the meaning.
#[derive(Debug, Deserialize, Serialize)]
pub struct DeclaredModelCard {
    pub target_column: String,
    pub prediction_column: String,
    #[serde(default)]
    pub score_column: Option<String>,
    #[serde(default)]
    pub positive_label: Option<String>,
    #[serde(default)]
    pub slice_by: Vec<String>,
    #[serde(default)]
    pub protected_attribute: Option<String>,
    #[serde(default)]
    pub thresholds: DeclaredFairnessThresholds,
    #[serde(default)]
    pub min_slice_size: Option<usize>,
    #[serde(default)]
    pub max_groups_per_column: Option<usize>,
    #[serde(default)]
    pub model: DeclaredModelMetadata,
}

/// One column of a `synthesize:` block: its public domain. Numeric columns declare bounds,
/// categorical columns declare their values. Never read from the data (bounds derived from the
/// data would themselves leak).
#[derive(Debug, Deserialize, Serialize)]
pub struct DeclaredSynthesizeColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: DeclaredSynthesizeKind,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub integer: bool,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredSynthesizeKind {
    Numeric,
    Categorical,
}

/// The `synthesize:` block of a `.triplex.yaml`: the differential-privacy settings for
/// `triplex synthesize`. `epsilon` has no default. Parsing only;
/// `triplex-anonymizer` owns the meaning.
#[derive(Debug, Deserialize, Serialize)]
pub struct DeclaredSynthesize {
    #[serde(default)]
    pub epsilon: Option<f64>,
    #[serde(default)]
    pub marginal_share: Option<f64>,
    #[serde(default)]
    pub bins: Option<usize>,
    /// `gaussian_copula` (the default) or `bayesian_network`.
    #[serde(default)]
    pub method: Option<String>,
    /// For `bayesian_network`: `chow_liu` (the default) or `k_parent`.
    #[serde(default)]
    pub structure: Option<String>,
    /// For `k_parent`: parents per column (1 to 3).
    #[serde(default)]
    pub max_parents: Option<usize>,
    /// For `bayesian_network`: share of epsilon spent choosing the structure.
    #[serde(default)]
    pub structure_share: Option<f64>,
    #[serde(default)]
    pub columns: Vec<DeclaredSynthesizeColumn>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DeclaredContract {
    pub version: u32,
    pub columns: Vec<DeclaredColumn>,
    #[serde(default)]
    pub driftguard: Option<DeclaredDriftGuardThresholds>,
    #[serde(default)]
    pub modelcard: Option<DeclaredModelCard>,
    #[serde(default)]
    pub synthesize: Option<DeclaredSynthesize>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValidationError {
    MissingColumn {
        name: String,
    },
    UnexpectedColumn {
        name: String,
    },
    TypeMismatch {
        name: String,
        expected: ColumnType,
        actual: ColumnType,
    },
    NullThresholdExceeded {
        name: String,
        max_percentage: f64,
        actual_percentage: f64,
    },
    TypeConsistencyBelowThreshold {
        name: String,
        min_percentage: f64,
        actual_percentage: f64,
    },
    BelowMinValue {
        name: String,
        min_value: f64,
        actual_value: f64,
    },
    AboveMaxValue {
        name: String,
        max_value: f64,
        actual_value: f64,
    },
    MeanOutOfTolerance {
        name: String,
        expected_mean: f64,
        epsilon: f64,
        actual_mean: f64,
    },
    /// Deliberately omits the offending value(s): a schema/validation report
    /// is a privacy-adjacent surface, so a mismatch is reported as a count,
    /// never as the raw cell content.
    PatternMismatch {
        name: String,
        pattern: String,
        non_conforming_count: usize,
        checked_count: usize,
    },
}

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
}

/// Parses a declarative schema contract from already-loaded YAML text — the
/// part of `load_contract` that doesn't need a local path, split out for a
/// caller with no filesystem (`apps/triplex-web`, where the browser already
/// has the contract's text from an upload or a generated string).
pub fn parse_contract(text: &str) -> Result<DeclaredContract> {
    serde_yaml_ng::from_str(text).map_err(|e| Error::Yaml(e.to_string()))
}

/// Loads a declarative schema contract from a local `.triplex.yaml` file.
pub fn load_contract(path: &Path) -> Result<DeclaredContract> {
    let text = std::fs::read_to_string(path)?;
    parse_contract(&text)
}

/// Converts an inferred schema into a declared-contract skeleton — every
/// column required, no thresholds or pattern set — ready to serialize as a
/// starting `.triplex.yaml` a user can hand-edit. Used by
/// `apps/triplex-web`'s "generate contract" action; the CLI's `triplex
/// schema` intentionally keeps emitting the raw inferred JSON instead,
/// since that's its machine-readable output for downstream tooling, not a
/// human-editable contract starting point.
pub fn to_declared_contract(inferred: &SchemaContract) -> DeclaredContract {
    DeclaredContract {
        version: 1,
        columns: inferred
            .columns
            .iter()
            .map(|c| DeclaredColumn {
                name: c.name.clone(),
                column_type: c.inferred_type,
                required: true,
                statistics: None,
                pattern: None,
            })
            .collect(),
        driftguard: None,
        modelcard: None,
        synthesize: None,
    }
}

/// Validates an inferred schema against a declared contract. Column order does
/// not matter; presence, requiredness, and type must match.
// Every `!(a <= b)`/`!(a >= b)` below is deliberate, not an oversight clippy
// caught: a NaN threshold must fail closed (reject everything), and
// `a > NaN`/`a < NaN` are always false, which would silently pass any
// column instead. `partial_cmp` would need the same NaN-handling logic
// spelled out longhand at each call site for no readability gain.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn validate(declared: &DeclaredContract, inferred: &SchemaContract) -> ValidationReport {
    let mut errors = Vec::new();

    for declared_col in &declared.columns {
        let Some(actual_col) = inferred
            .columns
            .iter()
            .find(|c| c.name == declared_col.name)
        else {
            if declared_col.required {
                errors.push(ValidationError::MissingColumn {
                    name: declared_col.name.clone(),
                });
            }
            continue;
        };

        if actual_col.inferred_type != declared_col.column_type {
            errors.push(ValidationError::TypeMismatch {
                name: declared_col.name.clone(),
                expected: declared_col.column_type,
                actual: actual_col.inferred_type,
            });
        }

        let stats = declared_col.statistics.as_ref();

        if let Some(max_percentage) = stats.and_then(|s| s.max_null_percentage) {
            // Not `>`: a NaN threshold must fail closed (reject everything), and
            // `x > NaN` is always false, which would silently pass any column.
            if !(actual_col.statistics.null_percentage <= max_percentage) {
                errors.push(ValidationError::NullThresholdExceeded {
                    name: declared_col.name.clone(),
                    max_percentage,
                    actual_percentage: actual_col.statistics.null_percentage,
                });
            }
        }

        if let Some(min_percentage) = stats.and_then(|s| s.min_type_consistency) {
            // Same fail-closed reasoning as the null-threshold check above.
            if !(actual_col.type_consistency_percentage >= min_percentage) {
                errors.push(ValidationError::TypeConsistencyBelowThreshold {
                    name: declared_col.name.clone(),
                    min_percentage,
                    actual_percentage: actual_col.type_consistency_percentage,
                });
            }
        }

        // A column with no parseable numbers has nothing to range-check —
        // that's what max_null_percentage/min_type_consistency are for.
        if let (Some(min_value), Some(actual_min)) = (
            stats.and_then(|s| s.min_value),
            actual_col.statistics.min_value,
        ) {
            if !(actual_min >= min_value) {
                errors.push(ValidationError::BelowMinValue {
                    name: declared_col.name.clone(),
                    min_value,
                    actual_value: actual_min,
                });
            }
        }

        if let (Some(max_value), Some(actual_max)) = (
            stats.and_then(|s| s.max_value),
            actual_col.statistics.max_value,
        ) {
            if !(actual_max <= max_value) {
                errors.push(ValidationError::AboveMaxValue {
                    name: declared_col.name.clone(),
                    max_value,
                    actual_value: actual_max,
                });
            }
        }

        // `mean_value` is None for Parquet (footer stats carry no sum), so this
        // only ever fires for CSV-sourced columns today.
        if let (Some(expected_mean), Some(actual_mean)) = (
            stats.and_then(|s| s.expected_mean),
            actual_col.statistics.mean_value,
        ) {
            let epsilon = stats.and_then(|s| s.mean_epsilon).unwrap_or(0.0);
            let low = expected_mean - epsilon;
            let high = expected_mean + epsilon;
            // Same fail-closed reasoning as the other thresholds: a NaN
            // expected_mean/epsilon makes this comparison false either way.
            if !(actual_mean >= low && actual_mean <= high) {
                errors.push(ValidationError::MeanOutOfTolerance {
                    name: declared_col.name.clone(),
                    expected_mean,
                    epsilon,
                    actual_mean,
                });
            }
        }
    }

    for actual_col in &inferred.columns {
        if !declared.columns.iter().any(|d| d.name == actual_col.name) {
            errors.push(ValidationError::UnexpectedColumn {
                name: actual_col.name.clone(),
            });
        }
    }

    ValidationReport {
        valid: errors.is_empty(),
        errors,
    }
}

/// Loads a declared contract and a local data file, then validates the one
/// against the other in a single step.
pub fn validate_path(
    schema_path: &Path,
    input_path: &Path,
    sample_rows: usize,
) -> Result<ValidationReport> {
    let declared = load_contract(schema_path)?;
    let inferred = crate::infer_schema_from_path(input_path, sample_rows)?;
    Ok(validate(&declared, &inferred))
}

/// Validates a `.csv`-shaped file whose actual field separator isn't a
/// comma against a declared contract — see
/// `crate::infer_schema_from_csv_with_delimiter` for why this bypasses the
/// usual extension-based dispatch.
pub fn validate_csv_with_delimiter(
    schema_path: &Path,
    input_path: &Path,
    delimiter: u8,
    sample_rows: usize,
) -> Result<ValidationReport> {
    let declared = load_contract(schema_path)?;
    let inferred = crate::infer_schema_from_csv_with_delimiter(input_path, delimiter, sample_rows)?;
    Ok(validate(&declared, &inferred))
}

/// Verifies a declared contract is internally well-formed — every declared
/// `pattern` compiles as a regex, and no column is declared twice —
/// independent of checking it against any actual data. Duplicate names are
/// rejected rather than silently allowed: `validate`/`check_patterns` look a
/// column up by `.find()`/`.position()` (first match), so a second entry for
/// the same column would be silently ignored rather than erroring — better
/// to catch that here, where it's built. Exists so a caller building a
/// contract programmatically (e.g. `apps/triplex-web`'s table interactions)
/// can catch an unusable contract before writing it out, rather than only
/// failing later when `check_patterns`/`validate` tries to use it.
pub fn verify_contract_well_formed(declared: &DeclaredContract) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for column in &declared.columns {
        if !seen.insert(column.name.as_str()) {
            return Err(Error::Validation(format!(
                "contract declares column \"{}\" more than once",
                column.name
            )));
        }
        let Some(pattern) = column.pattern.as_ref() else {
            continue;
        };
        Regex::new(pattern).map_err(|e| {
            Error::Validation(format!(
                "column \"{}\": invalid pattern regex: {e}",
                column.name
            ))
        })?;
    }
    Ok(())
}

/// Checks every declared `pattern` against every non-empty value in its
/// column across a full in-memory `Sample`. Separate from `validate`: this
/// needs the raw per-value data (not just aggregate stats), and it must
/// stay usable on a `Sample` alone (no file path) so the same check runs
/// against data already loaded in memory, e.g. anonymized output in the
/// browser (`apps/triplex-web`) with no local file to point at.
pub fn check_patterns(declared: &DeclaredContract, sample: &Sample) -> Result<ValidationReport> {
    let mut errors = Vec::new();

    for declared_col in &declared.columns {
        let Some(pattern) = declared_col.pattern.as_ref() else {
            continue;
        };
        let regex = Regex::new(pattern).map_err(|e| {
            Error::Yaml(format!(
                "invalid pattern for column \"{}\": {e}",
                declared_col.name
            ))
        })?;
        let Some(col_index) = sample.headers.iter().position(|h| h == &declared_col.name) else {
            continue; // MissingColumn is validate()'s job, not this one's
        };

        let mut checked_count = 0usize;
        let mut non_conforming_count = 0usize;
        for row in &sample.rows {
            let Some(value) = row.get(col_index) else {
                continue;
            };
            if value.is_empty() {
                continue; // an empty cell is null, not a pattern violation
            }
            checked_count += 1;
            if !regex.is_match(value) {
                non_conforming_count += 1;
            }
        }

        if non_conforming_count > 0 {
            errors.push(ValidationError::PatternMismatch {
                name: declared_col.name.clone(),
                pattern: pattern.clone(),
                non_conforming_count,
                checked_count,
            });
        }
    }

    Ok(ValidationReport {
        valid: errors.is_empty(),
        errors,
    })
}

/// Loads a declared contract and a local data file, then runs
/// `check_patterns` against it in one step.
pub fn check_patterns_path(schema_path: &Path, input_path: &Path) -> Result<ValidationReport> {
    let declared = load_contract(schema_path)?;
    let extension = input_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let sample = crate::convert::read_full(input_path, extension, None)?;
    check_patterns(&declared, &sample)
}

/// Byte-slice counterpart to `check_patterns_path` — for a caller with data
/// already in memory instead of a local file (e.g. `apps/triplex-web`
/// checking anonymized output against its source contract).
pub fn check_patterns_bytes(
    declared: &DeclaredContract,
    bytes: &[u8],
    extension: &str,
) -> Result<ValidationReport> {
    let sample = crate::convert::read_full_bytes(bytes, extension)?;
    check_patterns(declared, &sample)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColumnSchema;
    use triplex_io::ColumnStatistics;

    fn inferred(columns: &[(&str, ColumnType)]) -> SchemaContract {
        inferred_with_nulls(
            &columns
                .iter()
                .map(|(name, t)| (*name, *t, 0.0))
                .collect::<Vec<_>>(),
        )
    }

    fn inferred_with_nulls(columns: &[(&str, ColumnType, f64)]) -> SchemaContract {
        inferred_with_nulls_and_consistency(
            &columns
                .iter()
                .map(|(name, t, null_percentage)| (*name, *t, *null_percentage, 100.0))
                .collect::<Vec<_>>(),
        )
    }

    fn inferred_with_nulls_and_consistency(
        columns: &[(&str, ColumnType, f64, f64)],
    ) -> SchemaContract {
        SchemaContract {
            version: "0.1.0".into(),
            columns: columns
                .iter()
                .map(
                    |(name, t, null_percentage, type_consistency_percentage)| ColumnSchema {
                        name: (*name).to_string(),
                        inferred_type: *t,
                        semantic_type: None,
                        type_consistency_percentage: *type_consistency_percentage,
                        statistics: ColumnStatistics {
                            null_percentage: *null_percentage,
                            min_value: None,
                            max_value: None,
                            mean_value: None,
                        },
                        distribution: None,
                    },
                )
                .collect(),
        }
    }

    fn inferred_with_range(name: &str, t: ColumnType, min: f64, max: f64) -> SchemaContract {
        SchemaContract {
            version: "0.1.0".into(),
            columns: vec![ColumnSchema {
                name: name.to_string(),
                inferred_type: t,
                semantic_type: None,
                type_consistency_percentage: 100.0,
                statistics: ColumnStatistics {
                    null_percentage: 0.0,
                    min_value: Some(min),
                    max_value: Some(max),
                    mean_value: None,
                },
                distribution: None,
            }],
        }
    }

    fn inferred_with_mean(name: &str, t: ColumnType, mean: f64) -> SchemaContract {
        SchemaContract {
            version: "0.1.0".into(),
            columns: vec![ColumnSchema {
                name: name.to_string(),
                inferred_type: t,
                semantic_type: None,
                type_consistency_percentage: 100.0,
                statistics: ColumnStatistics {
                    null_percentage: 0.0,
                    min_value: None,
                    max_value: None,
                    mean_value: Some(mean),
                },
                distribution: None,
            }],
        }
    }

    fn declared(columns: Vec<DeclaredColumn>) -> DeclaredContract {
        DeclaredContract {
            version: 1,
            columns,
            driftguard: None,
            modelcard: None,
            synthesize: None,
        }
    }

    fn col(name: &str, column_type: ColumnType, required: bool) -> DeclaredColumn {
        DeclaredColumn {
            name: name.to_string(),
            column_type,
            required,
            statistics: None,
            pattern: None,
        }
    }

    fn sample(headers: &[&str], rows: &[Vec<&str>]) -> Sample {
        Sample {
            headers: headers.iter().map(|h| h.to_string()).collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|v| v.to_string()).collect())
                .collect(),
        }
    }

    #[test]
    fn matching_schema_is_valid() {
        let d = declared(vec![col("id", ColumnType::Integer, true)]);
        let i = inferred(&[("id", ColumnType::Integer)]);
        let report = validate(&d, &i);
        assert!(report.valid);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn missing_required_column_fails() {
        let d = declared(vec![col("id", ColumnType::Integer, true)]);
        let i = inferred(&[]);
        let report = validate(&d, &i);
        assert!(!report.valid);
        assert_eq!(
            report.errors,
            vec![ValidationError::MissingColumn { name: "id".into() }]
        );
    }

    #[test]
    fn missing_optional_column_is_fine() {
        let d = declared(vec![col("id", ColumnType::Integer, false)]);
        let i = inferred(&[]);
        let report = validate(&d, &i);
        assert!(report.valid);
    }

    #[test]
    fn type_mismatch_fails() {
        let d = declared(vec![col("id", ColumnType::Integer, true)]);
        let i = inferred(&[("id", ColumnType::String)]);
        let report = validate(&d, &i);
        assert_eq!(
            report.errors,
            vec![ValidationError::TypeMismatch {
                name: "id".into(),
                expected: ColumnType::Integer,
                actual: ColumnType::String,
            }]
        );
    }

    #[test]
    fn unexpected_column_fails() {
        let d = declared(vec![]);
        let i = inferred(&[("extra", ColumnType::String)]);
        let report = validate(&d, &i);
        assert_eq!(
            report.errors,
            vec![ValidationError::UnexpectedColumn {
                name: "extra".into()
            }]
        );
    }

    #[test]
    fn null_percentage_within_threshold_is_fine() {
        let mut d = declared(vec![col("id", ColumnType::Integer, true)]);
        d.columns[0].statistics_mut().max_null_percentage = Some(50.0);
        let i = inferred_with_nulls(&[("id", ColumnType::Integer, 25.0)]);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn null_percentage_over_threshold_fails() {
        let mut d = declared(vec![col("id", ColumnType::Integer, true)]);
        d.columns[0].statistics_mut().max_null_percentage = Some(10.0);
        let i = inferred_with_nulls(&[("id", ColumnType::Integer, 25.0)]);
        assert_eq!(
            validate(&d, &i).errors,
            vec![ValidationError::NullThresholdExceeded {
                name: "id".into(),
                max_percentage: 10.0,
                actual_percentage: 25.0,
            }]
        );
    }

    #[test]
    fn no_threshold_declared_means_any_null_rate_is_fine() {
        let d = declared(vec![col("id", ColumnType::Integer, true)]);
        let i = inferred_with_nulls(&[("id", ColumnType::Integer, 99.0)]);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn nan_null_threshold_fails_closed_instead_of_silently_passing() {
        let mut d = declared(vec![col("id", ColumnType::Integer, true)]);
        d.columns[0].statistics_mut().max_null_percentage = Some(f64::NAN);
        let i = inferred_with_nulls(&[("id", ColumnType::Integer, 0.0)]);
        assert!(!validate(&d, &i).valid);
    }

    #[test]
    fn nan_type_consistency_threshold_fails_closed_instead_of_silently_passing() {
        let mut d = declared(vec![col("id", ColumnType::Integer, true)]);
        d.columns[0].statistics_mut().min_type_consistency = Some(f64::NAN);
        let i = inferred_with_nulls_and_consistency(&[("id", ColumnType::Integer, 0.0, 100.0)]);
        assert!(!validate(&d, &i).valid);
    }

    #[test]
    fn type_consistency_within_threshold_is_fine() {
        let mut d = declared(vec![col("qty", ColumnType::String, true)]);
        d.columns[0].statistics_mut().min_type_consistency = Some(50.0);
        let i = inferred_with_nulls_and_consistency(&[("qty", ColumnType::String, 0.0, 75.0)]);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn type_consistency_below_threshold_fails() {
        let mut d = declared(vec![col("qty", ColumnType::String, true)]);
        d.columns[0].statistics_mut().min_type_consistency = Some(90.0);
        let i = inferred_with_nulls_and_consistency(&[("qty", ColumnType::String, 0.0, 75.0)]);
        assert_eq!(
            validate(&d, &i).errors,
            vec![ValidationError::TypeConsistencyBelowThreshold {
                name: "qty".into(),
                min_percentage: 90.0,
                actual_percentage: 75.0,
            }]
        );
    }

    #[test]
    fn no_consistency_threshold_declared_means_any_mix_is_fine() {
        let d = declared(vec![col("qty", ColumnType::String, true)]);
        let i = inferred_with_nulls_and_consistency(&[("qty", ColumnType::String, 0.0, 1.0)]);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn value_within_range_is_fine() {
        let mut d = declared(vec![col("age", ColumnType::Integer, true)]);
        d.columns[0].statistics_mut().min_value = Some(0.0);
        d.columns[0].statistics_mut().max_value = Some(120.0);
        let i = inferred_with_range("age", ColumnType::Integer, 0.0, 99.0);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn value_below_min_fails() {
        let mut d = declared(vec![col("age", ColumnType::Integer, true)]);
        d.columns[0].statistics_mut().min_value = Some(0.0);
        let i = inferred_with_range("age", ColumnType::Integer, -5.0, 99.0);
        assert_eq!(
            validate(&d, &i).errors,
            vec![ValidationError::BelowMinValue {
                name: "age".into(),
                min_value: 0.0,
                actual_value: -5.0,
            }]
        );
    }

    #[test]
    fn value_above_max_fails() {
        let mut d = declared(vec![col("age", ColumnType::Integer, true)]);
        d.columns[0].statistics_mut().max_value = Some(120.0);
        let i = inferred_with_range("age", ColumnType::Integer, 0.0, 999.0);
        assert_eq!(
            validate(&d, &i).errors,
            vec![ValidationError::AboveMaxValue {
                name: "age".into(),
                max_value: 120.0,
                actual_value: 999.0,
            }]
        );
    }

    #[test]
    fn no_range_declared_means_any_value_is_fine() {
        let d = declared(vec![col("age", ColumnType::Integer, true)]);
        let i = inferred_with_range("age", ColumnType::Integer, -999.0, 999.0);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn a_column_with_no_numeric_values_has_nothing_to_range_check() {
        let mut d = declared(vec![col("name", ColumnType::String, true)]);
        d.columns[0].statistics_mut().min_value = Some(0.0);
        d.columns[0].statistics_mut().max_value = Some(120.0);
        let i = inferred(&[("name", ColumnType::String)]); // min_value/max_value both None
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn nan_min_value_fails_closed_instead_of_silently_passing() {
        let mut d = declared(vec![col("age", ColumnType::Integer, true)]);
        d.columns[0].statistics_mut().min_value = Some(f64::NAN);
        let i = inferred_with_range("age", ColumnType::Integer, 0.0, 99.0);
        assert!(!validate(&d, &i).valid);
    }

    #[test]
    fn nan_max_value_fails_closed_instead_of_silently_passing() {
        let mut d = declared(vec![col("age", ColumnType::Integer, true)]);
        d.columns[0].statistics_mut().max_value = Some(f64::NAN);
        let i = inferred_with_range("age", ColumnType::Integer, 0.0, 99.0);
        assert!(!validate(&d, &i).valid);
    }

    #[test]
    fn mean_within_epsilon_is_fine() {
        let mut d = declared(vec![col("score", ColumnType::Float, true)]);
        d.columns[0].statistics_mut().expected_mean = Some(50.0);
        d.columns[0].statistics_mut().mean_epsilon = Some(5.0);
        let i = inferred_with_mean("score", ColumnType::Float, 53.0);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn mean_outside_epsilon_fails() {
        let mut d = declared(vec![col("score", ColumnType::Float, true)]);
        d.columns[0].statistics_mut().expected_mean = Some(50.0);
        d.columns[0].statistics_mut().mean_epsilon = Some(5.0);
        let i = inferred_with_mean("score", ColumnType::Float, 60.0);
        assert_eq!(
            validate(&d, &i).errors,
            vec![ValidationError::MeanOutOfTolerance {
                name: "score".into(),
                expected_mean: 50.0,
                epsilon: 5.0,
                actual_mean: 60.0,
            }]
        );
    }

    #[test]
    fn omitted_epsilon_defaults_to_an_exact_match() {
        let mut d = declared(vec![col("score", ColumnType::Float, true)]);
        d.columns[0].statistics_mut().expected_mean = Some(50.0);
        // No mean_epsilon set.
        let exact = inferred_with_mean("score", ColumnType::Float, 50.0);
        assert!(validate(&d, &exact).valid);

        let off_by_a_little = inferred_with_mean("score", ColumnType::Float, 50.001);
        assert!(!validate(&d, &off_by_a_little).valid);
    }

    #[test]
    fn no_expected_mean_declared_means_any_mean_is_fine() {
        let d = declared(vec![col("score", ColumnType::Float, true)]);
        let i = inferred_with_mean("score", ColumnType::Float, 999_999.0);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn a_column_with_no_numeric_values_has_nothing_to_check_the_mean_of() {
        let mut d = declared(vec![col("name", ColumnType::String, true)]);
        d.columns[0].statistics_mut().expected_mean = Some(50.0);
        let i = inferred(&[("name", ColumnType::String)]); // mean_value is None
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn nan_expected_mean_fails_closed_instead_of_silently_passing() {
        let mut d = declared(vec![col("score", ColumnType::Float, true)]);
        d.columns[0].statistics_mut().expected_mean = Some(f64::NAN);
        let i = inferred_with_mean("score", ColumnType::Float, 50.0);
        assert!(!validate(&d, &i).valid);
    }

    #[test]
    fn loads_and_validates_the_synthetic_fixtures() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");

        let ok_report = validate_path(
            &fixtures.join("sample.contract.yaml"),
            &fixtures.join("sample.csv"),
            100,
        )
        .expect("valid fixture pair should load");
        assert!(ok_report.valid, "expected valid, got {ok_report:?}");

        let broken_report = validate_path(
            &fixtures.join("sample.broken.contract.yaml"),
            &fixtures.join("sample.csv"),
            100,
        )
        .expect("broken fixture pair should still load");
        assert!(!broken_report.valid);
        assert!(broken_report.errors.iter().any(
            |e| matches!(e, ValidationError::NullThresholdExceeded { name, .. } if name == "price")
        ));
        assert!(broken_report
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::AboveMaxValue { name, .. } if name == "price")));
        assert!(broken_report.errors.iter().any(
            |e| matches!(e, ValidationError::MeanOutOfTolerance { name, .. } if name == "price")
        ));
    }

    #[test]
    fn every_value_matching_the_pattern_is_valid() {
        let mut d = declared(vec![col("code", ColumnType::String, true)]);
        d.columns[0].pattern = Some("^[A-Z]{2}-[0-9]{3}$".to_string());
        let s = sample(&["code"], &[vec!["AB-123"], vec!["ZZ-999"]]);
        assert!(check_patterns(&d, &s).unwrap().valid);
    }

    #[test]
    fn non_conforming_values_fail_without_leaking_the_raw_value() {
        let mut d = declared(vec![col("code", ColumnType::String, true)]);
        d.columns[0].pattern = Some("^[A-Z]{2}-[0-9]{3}$".to_string());
        let s = sample(
            &["code"],
            &[vec!["AB-123"], vec!["not-a-code"], vec!["also-bad"]],
        );
        let report = check_patterns(&d, &s).unwrap();
        assert_eq!(
            report.errors,
            vec![ValidationError::PatternMismatch {
                name: "code".into(),
                pattern: "^[A-Z]{2}-[0-9]{3}$".into(),
                non_conforming_count: 2,
                checked_count: 3,
            }]
        );
    }

    #[test]
    fn no_pattern_declared_means_any_value_is_fine() {
        let d = declared(vec![col("code", ColumnType::String, true)]);
        let s = sample(&["code"], &[vec!["anything at all"]]);
        assert!(check_patterns(&d, &s).unwrap().valid);
    }

    #[test]
    fn empty_cells_are_null_not_a_pattern_violation() {
        let mut d = declared(vec![col("code", ColumnType::String, true)]);
        d.columns[0].pattern = Some("^[A-Z]+$".to_string());
        let s = sample(&["code"], &[vec![""], vec!["AB"]]);
        assert!(check_patterns(&d, &s).unwrap().valid);
    }

    #[test]
    fn an_invalid_regex_in_the_contract_is_a_clear_error_not_a_panic() {
        let mut d = declared(vec![col("code", ColumnType::String, true)]);
        d.columns[0].pattern = Some("[unterminated".to_string());
        let s = sample(&["code"], &[vec!["x"]]);
        assert!(check_patterns(&d, &s).is_err());
    }

    #[test]
    fn verify_contract_well_formed_rejects_an_uncompilable_pattern() {
        let mut d = declared(vec![col("code", ColumnType::String, true)]);
        d.columns[0].pattern = Some("[unterminated".to_string());
        let err = verify_contract_well_formed(&d).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn verify_contract_well_formed_accepts_a_compilable_pattern_or_none_at_all() {
        let mut d = declared(vec![
            col("code", ColumnType::String, true),
            col("name", ColumnType::String, true),
        ]);
        d.columns[0].pattern = Some("^[A-Z]+$".to_string());
        // `name` has no pattern at all — nothing to check, not a failure.
        assert!(verify_contract_well_formed(&d).is_ok());
    }

    #[test]
    fn verify_contract_well_formed_rejects_a_column_declared_twice() {
        let d = declared(vec![
            col("code", ColumnType::String, true),
            col("code", ColumnType::String, true),
        ]);
        let err = verify_contract_well_formed(&d).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn a_column_missing_from_the_sample_has_nothing_to_pattern_check() {
        let mut d = declared(vec![col("code", ColumnType::String, true)]);
        d.columns[0].pattern = Some("^[A-Z]+$".to_string());
        let s = sample(&["other"], &[vec!["x"]]);
        assert!(check_patterns(&d, &s).unwrap().valid);
    }

    #[test]
    fn check_patterns_bytes_matches_the_path_based_check() {
        let declared = load_contract(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/sample.pattern.broken.contract.yaml"),
        )
        .unwrap();
        let bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/sample.csv"),
        )
        .unwrap();

        let report = check_patterns_bytes(&declared, &bytes, "csv").unwrap();
        assert!(!report.valid);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::PatternMismatch { name, .. } if name == "name")));
    }

    #[test]
    fn check_patterns_path_reads_and_checks_the_synthetic_fixture() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");

        let ok_report = check_patterns_path(
            &fixtures.join("sample.pattern.contract.yaml"),
            &fixtures.join("sample.csv"),
        )
        .expect("valid fixture pair should load");
        assert!(ok_report.valid, "expected valid, got {ok_report:?}");

        let broken_report = check_patterns_path(
            &fixtures.join("sample.pattern.broken.contract.yaml"),
            &fixtures.join("sample.csv"),
        )
        .expect("broken fixture pair should still load");
        assert!(!broken_report.valid);
        assert!(broken_report
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::PatternMismatch { name, .. } if name == "name")));
    }
}
