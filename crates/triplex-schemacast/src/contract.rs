//! Declarative `.triplex.yaml` schema contracts and validation of an inferred
//! schema against one. No coercion modes yet — a column either matches the
//! declared contract or it doesn't.

use crate::{ColumnType, SchemaContract};
use serde::{Deserialize, Serialize};
use std::path::Path;
use triplex_core::{Error, Result};

fn default_required() -> bool {
    true
}

/// Statistical thresholds a declared column may be checked against. Grouped
/// separately from identity/type (name, column_type, required) since these
/// are all optional, orthogonal checks against the inferred column's
/// `ColumnStatistics` — see `triplex_io::ColumnStatistics`.
#[derive(Debug, Default, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct DeclaredColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: ColumnType,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default)]
    pub statistics: Option<DeclaredColumnStatistics>,
}

impl DeclaredColumn {
    /// Gets (or lazily creates) this column's statistics thresholds — handy
    /// for building a contract programmatically rather than from YAML.
    pub fn statistics_mut(&mut self) -> &mut DeclaredColumnStatistics {
        self.statistics
            .get_or_insert_with(DeclaredColumnStatistics::default)
    }
}

#[derive(Debug, Deserialize)]
pub struct DeclaredContract {
    pub version: u32,
    pub columns: Vec<DeclaredColumn>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValidationError {
    MissingColumn { name: String },
    UnexpectedColumn { name: String },
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
}

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
}

/// Loads a declarative schema contract from a local `.triplex.yaml` file.
pub fn load_contract(path: &Path) -> Result<DeclaredContract> {
    let text = std::fs::read_to_string(path)?;
    serde_yaml_ng::from_str(&text).map_err(|e| Error::Yaml(e.to_string()))
}

/// Validates an inferred schema against a declared contract. Column order does
/// not matter; presence, requiredness, and type must match.
pub fn validate(declared: &DeclaredContract, inferred: &SchemaContract) -> ValidationReport {
    let mut errors = Vec::new();

    for declared_col in &declared.columns {
        let Some(actual_col) = inferred.columns.iter().find(|c| c.name == declared_col.name)
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
                .map(|(name, t, null_percentage, type_consistency_percentage)| ColumnSchema {
                    name: (*name).to_string(),
                    inferred_type: *t,
                    type_consistency_percentage: *type_consistency_percentage,
                    statistics: ColumnStatistics {
                        null_percentage: *null_percentage,
                        min_value: None,
                        max_value: None,
                        mean_value: None,
                    },
                })
                .collect(),
        }
    }

    fn inferred_with_range(name: &str, t: ColumnType, min: f64, max: f64) -> SchemaContract {
        SchemaContract {
            version: "0.1.0".into(),
            columns: vec![ColumnSchema {
                name: name.to_string(),
                inferred_type: t,
                type_consistency_percentage: 100.0,
                statistics: ColumnStatistics {
                    null_percentage: 0.0,
                    min_value: Some(min),
                    max_value: Some(max),
                    mean_value: None,
                },
            }],
        }
    }

    fn inferred_with_mean(name: &str, t: ColumnType, mean: f64) -> SchemaContract {
        SchemaContract {
            version: "0.1.0".into(),
            columns: vec![ColumnSchema {
                name: name.to_string(),
                inferred_type: t,
                type_consistency_percentage: 100.0,
                statistics: ColumnStatistics {
                    null_percentage: 0.0,
                    min_value: None,
                    max_value: None,
                    mean_value: Some(mean),
                },
            }],
        }
    }

    fn declared(columns: Vec<DeclaredColumn>) -> DeclaredContract {
        DeclaredContract {
            version: 1,
            columns,
        }
    }

    fn col(name: &str, column_type: ColumnType, required: bool) -> DeclaredColumn {
        DeclaredColumn {
            name: name.to_string(),
            column_type,
            required,
            statistics: None,
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
        assert!(broken_report
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::NullThresholdExceeded { name, .. } if name == "price")));
        assert!(broken_report
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::AboveMaxValue { name, .. } if name == "price")));
        assert!(broken_report
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::MeanOutOfTolerance { name, .. } if name == "price")));
    }
}
