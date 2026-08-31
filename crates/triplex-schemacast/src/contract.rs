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

#[derive(Debug, Deserialize)]
pub struct DeclaredColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: ColumnType,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default)]
    pub max_null_percentage: Option<f64>,
    #[serde(default)]
    pub min_type_consistency: Option<f64>,
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

        if let Some(max_percentage) = declared_col.max_null_percentage {
            // Not `>`: a NaN threshold must fail closed (reject everything), and
            // `x > NaN` is always false, which would silently pass any column.
            if !(actual_col.null_percentage <= max_percentage) {
                errors.push(ValidationError::NullThresholdExceeded {
                    name: declared_col.name.clone(),
                    max_percentage,
                    actual_percentage: actual_col.null_percentage,
                });
            }
        }

        if let Some(min_percentage) = declared_col.min_type_consistency {
            // Same fail-closed reasoning as the null-threshold check above.
            if !(actual_col.type_consistency_percentage >= min_percentage) {
                errors.push(ValidationError::TypeConsistencyBelowThreshold {
                    name: declared_col.name.clone(),
                    min_percentage,
                    actual_percentage: actual_col.type_consistency_percentage,
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
                    null_percentage: *null_percentage,
                    type_consistency_percentage: *type_consistency_percentage,
                })
                .collect(),
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
            max_null_percentage: None,
            min_type_consistency: None,
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
        d.columns[0].max_null_percentage = Some(50.0);
        let i = inferred_with_nulls(&[("id", ColumnType::Integer, 25.0)]);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn null_percentage_over_threshold_fails() {
        let mut d = declared(vec![col("id", ColumnType::Integer, true)]);
        d.columns[0].max_null_percentage = Some(10.0);
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
        d.columns[0].max_null_percentage = Some(f64::NAN);
        let i = inferred_with_nulls(&[("id", ColumnType::Integer, 0.0)]);
        assert!(!validate(&d, &i).valid);
    }

    #[test]
    fn nan_type_consistency_threshold_fails_closed_instead_of_silently_passing() {
        let mut d = declared(vec![col("id", ColumnType::Integer, true)]);
        d.columns[0].min_type_consistency = Some(f64::NAN);
        let i = inferred_with_nulls_and_consistency(&[("id", ColumnType::Integer, 0.0, 100.0)]);
        assert!(!validate(&d, &i).valid);
    }

    #[test]
    fn type_consistency_within_threshold_is_fine() {
        let mut d = declared(vec![col("qty", ColumnType::String, true)]);
        d.columns[0].min_type_consistency = Some(50.0);
        let i = inferred_with_nulls_and_consistency(&[("qty", ColumnType::String, 0.0, 75.0)]);
        assert!(validate(&d, &i).valid);
    }

    #[test]
    fn type_consistency_below_threshold_fails() {
        let mut d = declared(vec![col("qty", ColumnType::String, true)]);
        d.columns[0].min_type_consistency = Some(90.0);
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
    }
}
