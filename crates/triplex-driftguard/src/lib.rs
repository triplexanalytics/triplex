//! DriftGuard: verifier half of the Producer-Verifier flywheel
//! (the design notes). Diffs two SchemaCast-inferred
//! schemas — a baseline and a candidate — for schema regressions (a column
//! removed, or its type changed) and statistical drift (null rate, mean, or
//! type-consistency moving beyond a threshold).
//!
//! Tier-independent by design:
//! DriftGuard diffs whatever stats an artifact carries, regardless of which
//! Anonymizer tier — or no Anonymizer at all — produced the candidate file.
//! It has no dependency on `triplex-anonymizer`.

mod report_html;
mod report_script;

pub use report_html::render_html;
pub use report_script::render_python_script;
// Re-exported so a caller (e.g. `apps/triplex-cli`) can read/construct a
// `DriftProfile`'s `risk_assessment` without taking its own direct
// dependency on `triplex-datasetcard` -- same facade pattern
// `triplex-anonymizer` already uses for the same type.
pub use triplex_datasetcard::RiskAssessment;

use serde::{Deserialize, Serialize};
use std::path::Path;
use triplex_core::{Error, Result};
use triplex_schemacast::{ColumnSchema, ColumnType, SchemaContract};

/// Statistical-drift sensitivity. Schema regressions (a column removed, or
/// its type changed) are always reported and always set
/// `DriftReport::has_regression` regardless of these thresholds — there is
/// no "acceptable amount" of a column disappearing.
#[derive(Debug, Clone, Copy)]
pub struct DriftThresholds {
    /// Percentage-point change in null rate that counts as drift.
    pub max_null_percentage_delta: f64,
    /// Relative change in mean (`|delta| / |baseline_mean|`) that counts as
    /// drift.
    pub max_mean_relative_delta: f64,
    /// Percentage-point change in type-consistency that counts as drift.
    pub max_type_consistency_delta: f64,
}

impl Default for DriftThresholds {
    fn default() -> Self {
        Self {
            max_null_percentage_delta: 5.0,
            max_mean_relative_delta: 0.10,
            max_type_consistency_delta: 5.0,
        }
    }
}

impl DriftThresholds {
    /// Layers optional overrides -- e.g. a `.triplex.yaml` contract's
    /// `driftguard:` block, then a CLI flag on top of that -- onto the
    /// built-in defaults. `None` in any field keeps this crate's own
    /// default for it; the caller decides precedence by what it passes
    /// (typically `cli_flag.or(config_value)`).
    pub fn with_overrides(
        max_null_percentage_delta: Option<f64>,
        max_mean_relative_delta: Option<f64>,
        max_type_consistency_delta: Option<f64>,
    ) -> Self {
        let defaults = Self::default();
        Self {
            max_null_percentage_delta: max_null_percentage_delta
                .unwrap_or(defaults.max_null_percentage_delta),
            max_mean_relative_delta: max_mean_relative_delta
                .unwrap_or(defaults.max_mean_relative_delta),
            max_type_consistency_delta: max_type_consistency_delta
                .unwrap_or(defaults.max_type_consistency_delta),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DriftFinding {
    ColumnAdded {
        name: String,
    },
    ColumnRemoved {
        name: String,
    },
    TypeChanged {
        name: String,
        baseline_type: ColumnType,
        candidate_type: ColumnType,
    },
    NullRateDrift {
        name: String,
        baseline_percentage: f64,
        candidate_percentage: f64,
        delta: f64,
        threshold: f64,
    },
    MeanDrift {
        name: String,
        baseline_mean: f64,
        candidate_mean: f64,
        relative_delta: f64,
        threshold: f64,
    },
    TypeConsistencyDrift {
        name: String,
        baseline_percentage: f64,
        candidate_percentage: f64,
        delta: f64,
        threshold: f64,
    },
    /// The baseline and candidate carried the same `RiskModel` (e.g. both
    /// `KAnonymity`) and the candidate's re-identification exposure got
    /// worse -- a smaller k-anonymity/l-diversity, or a larger t-closeness/
    /// beta-likeness/delta-disclosure. Only fires when both sides actually
    /// have a `RiskAssessment` (see `DriftProfile`); a mismatched pair of
    /// risk models isn't comparable and is silently skipped rather than
    /// reported. Warning-level (drift, not regression) for now -- see
    /// `is_regression`'s doc comment.
    RiskDegraded {
        model: String,
        baseline: f64,
        candidate: f64,
    },
}

impl DriftFinding {
    /// A schema-breaking change: no threshold makes this acceptable, and it
    /// always sets `DriftReport::has_regression`. Everything else is
    /// statistical drift, gated by `DriftThresholds`.
    pub fn is_regression(&self) -> bool {
        matches!(
            self,
            DriftFinding::ColumnRemoved { .. } | DriftFinding::TypeChanged { .. }
        )
    }
}

#[derive(Debug, Serialize)]
pub struct DriftReport {
    pub has_regression: bool,
    pub has_drift: bool,
    pub findings: Vec<DriftFinding>,
}

fn column_by_name<'a>(schema: &'a SchemaContract, name: &str) -> Option<&'a ColumnSchema> {
    schema.columns.iter().find(|c| c.name == name)
}

/// The `baseline.profile.json` artifact the design notes
/// the design notes's Producer-Verifier flywheel describes Anonymizer as
/// auto-minting -- a schema profile plus whatever `RiskAssessment` the
/// producing run already computed (e.g. `triplex-anonymizer`'s
/// `AnonymizeReport::risk_assessment`), so DriftGuard can diff both without
/// re-inferring the original data or re-deriving a quasi-identifier's risk
/// score from scratch (which needs recipe knowledge DriftGuard doesn't
/// have). Deliberately *not* shared as a Rust type with `triplex-anonymizer`
/// -- see this crate's module doc on why DriftGuard has no dependency on
/// it; the two crates agree on this shape by convention, not by importing
/// each other.
#[derive(Debug, Serialize, Deserialize)]
pub struct DriftProfile {
    pub schema: SchemaContract,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_assessment: Option<RiskAssessment>,
}

/// Loads a `DriftProfile` written by `triplex anonymize --profile-out`, or
/// falls back to accepting a bare `triplex schema`/`SchemaContract` JSON
/// output directly (`risk_assessment: None` in that case) -- the two shapes
/// are structurally distinct (`{"schema": ..., "risk_assessment": ...}` vs.
/// `{"version": ..., "columns": ...}`), so there's no ambiguity in trying
/// the richer shape first.
pub fn load_profile(path: &Path) -> Result<DriftProfile> {
    let text = std::fs::read_to_string(path)?;
    if let Ok(profile) = serde_json::from_str::<DriftProfile>(&text) {
        return Ok(profile);
    }
    serde_json::from_str::<SchemaContract>(&text)
        .map(|schema| DriftProfile {
            schema,
            risk_assessment: None,
        })
        .map_err(|e| Error::Validation(format!("invalid drift profile/schema JSON: {e}")))
}

/// Compares a same-model `RiskAssessment` pair and reports a finding only
/// when the candidate's re-identification exposure got strictly worse.
/// Mismatched models (e.g. baseline scored `KAnonymity`, candidate scored
/// `TCloseness`) aren't comparable and are silently skipped.
fn risk_degradation(baseline: &RiskAssessment, candidate: &RiskAssessment) -> Option<DriftFinding> {
    use RiskAssessment::*;
    let (model, base_value, cand_value, worse) = match (baseline, candidate) {
        (KAnonymity(b), KAnonymity(c)) => ("k_anonymity", *b as f64, *c as f64, c < b),
        (LDiversity(b), LDiversity(c)) => ("l_diversity", *b as f64, *c as f64, c < b),
        (TCloseness(b), TCloseness(c)) => ("t_closeness", *b, *c, c > b),
        (BetaLikeness(b), BetaLikeness(c)) => ("beta_likeness", *b, *c, c > b),
        (DeltaDisclosure(b), DeltaDisclosure(c)) => ("delta_disclosure", *b, *c, c > b),
        _ => return None,
    };
    if !worse {
        return None;
    }
    Some(DriftFinding::RiskDegraded {
        model: model.to_string(),
        baseline: base_value,
        candidate: cand_value,
    })
}

/// Diffs an already-inferred baseline and candidate schema. Column order
/// does not matter, matching `triplex_schemacast::validate`'s convention.
pub fn compare_contracts(
    baseline: &SchemaContract,
    candidate: &SchemaContract,
    thresholds: &DriftThresholds,
) -> DriftReport {
    let mut findings = Vec::new();

    for base_col in &baseline.columns {
        let Some(cand_col) = column_by_name(candidate, &base_col.name) else {
            findings.push(DriftFinding::ColumnRemoved {
                name: base_col.name.clone(),
            });
            continue;
        };

        if cand_col.inferred_type != base_col.inferred_type {
            findings.push(DriftFinding::TypeChanged {
                name: base_col.name.clone(),
                baseline_type: base_col.inferred_type,
                candidate_type: cand_col.inferred_type,
            });
            // A changed type makes every other stat comparison meaningless
            // (e.g. diffing a Float column's mean against a String column's
            // absent one) -- skip them once the type itself has moved.
            continue;
        }

        let null_delta =
            (cand_col.statistics.null_percentage - base_col.statistics.null_percentage).abs();
        if null_delta > thresholds.max_null_percentage_delta {
            findings.push(DriftFinding::NullRateDrift {
                name: base_col.name.clone(),
                baseline_percentage: base_col.statistics.null_percentage,
                candidate_percentage: cand_col.statistics.null_percentage,
                delta: null_delta,
                threshold: thresholds.max_null_percentage_delta,
            });
        }

        let consistency_delta =
            (cand_col.type_consistency_percentage - base_col.type_consistency_percentage).abs();
        if consistency_delta > thresholds.max_type_consistency_delta {
            findings.push(DriftFinding::TypeConsistencyDrift {
                name: base_col.name.clone(),
                baseline_percentage: base_col.type_consistency_percentage,
                candidate_percentage: cand_col.type_consistency_percentage,
                delta: consistency_delta,
                threshold: thresholds.max_type_consistency_delta,
            });
        }

        // ponytail: a baseline mean of exactly 0.0 makes a relative delta
        // undefined (division by zero), so that column's mean drift is
        // skipped rather than reported -- upgrade to an absolute-delta
        // fallback if a real zero-mean column needs coverage.
        if let (Some(base_mean), Some(cand_mean)) = (
            base_col.statistics.mean_value,
            cand_col.statistics.mean_value,
        ) {
            if base_mean != 0.0 {
                let relative_delta = ((cand_mean - base_mean) / base_mean).abs();
                if relative_delta > thresholds.max_mean_relative_delta {
                    findings.push(DriftFinding::MeanDrift {
                        name: base_col.name.clone(),
                        baseline_mean: base_mean,
                        candidate_mean: cand_mean,
                        relative_delta,
                        threshold: thresholds.max_mean_relative_delta,
                    });
                }
            }
        }
    }

    for cand_col in &candidate.columns {
        if column_by_name(baseline, &cand_col.name).is_none() {
            findings.push(DriftFinding::ColumnAdded {
                name: cand_col.name.clone(),
            });
        }
    }

    let has_regression = findings.iter().any(DriftFinding::is_regression);
    DriftReport {
        has_regression,
        has_drift: !findings.is_empty(),
        findings,
    }
}

/// `compare_contracts` plus an optional risk-score diff -- only runs when
/// both sides actually have a `RiskAssessment` (typically because both came
/// from a `DriftProfile`, i.e. two Anonymizer runs, not a bare data file:
/// scoring a bare candidate file's risk needs recipe/quasi-identifier
/// knowledge DriftGuard doesn't have).
pub fn compare_with_risk(
    baseline_schema: &SchemaContract,
    baseline_risk: Option<&RiskAssessment>,
    candidate_schema: &SchemaContract,
    candidate_risk: Option<&RiskAssessment>,
    thresholds: &DriftThresholds,
) -> DriftReport {
    let mut report = compare_contracts(baseline_schema, candidate_schema, thresholds);
    if let (Some(base), Some(cand)) = (baseline_risk, candidate_risk) {
        if let Some(finding) = risk_degradation(base, cand) {
            report.findings.push(finding);
            report.has_drift = true;
        }
    }
    report
}

/// Reads two local data files and diffs their inferred schemas in one step
/// -- `.csv`/`.tsv`/`.jsonl`/`.parquet`, whatever `triplex-schemacast`
/// already ingests. Loading a previously-saved `triplex schema` JSON as the
/// baseline (skip re-reading the original data file) is a natural
/// follow-up, deferred here: it needs `Deserialize` added to
/// `SchemaContract`'s statistics/distribution types, which today only
/// derive `Serialize`.
pub fn compare_paths(
    baseline_path: &Path,
    candidate_path: &Path,
    sample_rows: usize,
    thresholds: &DriftThresholds,
) -> Result<DriftReport> {
    let baseline = triplex_schemacast::infer_schema_from_path(baseline_path, sample_rows)?;
    let candidate = triplex_schemacast::infer_schema_from_path(candidate_path, sample_rows)?;
    Ok(compare_contracts(&baseline, &candidate, thresholds))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(columns: Vec<ColumnSchema>) -> SchemaContract {
        SchemaContract {
            version: "0.1.0".to_string(),
            columns,
        }
    }

    fn column(name: &str, inferred_type: ColumnType, mean: Option<f64>) -> ColumnSchema {
        ColumnSchema {
            name: name.to_string(),
            inferred_type,
            semantic_type: None,
            type_consistency_percentage: 100.0,
            statistics: triplex_io::ColumnStatistics {
                null_percentage: 0.0,
                min_value: mean,
                max_value: mean,
                mean_value: mean,
            },
            distribution: None,
        }
    }

    #[test]
    fn identical_schemas_report_no_drift() {
        let baseline = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let candidate = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let report = compare_contracts(&baseline, &candidate, &DriftThresholds::default());
        assert!(!report.has_regression);
        assert!(!report.has_drift);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn removed_column_is_a_regression() {
        let baseline = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let candidate = schema(vec![]);
        let report = compare_contracts(&baseline, &candidate, &DriftThresholds::default());
        assert!(report.has_regression);
        assert_eq!(
            report.findings,
            vec![DriftFinding::ColumnRemoved {
                name: "age".to_string()
            }]
        );
    }

    #[test]
    fn added_column_is_drift_not_regression() {
        let baseline = schema(vec![]);
        let candidate = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let report = compare_contracts(&baseline, &candidate, &DriftThresholds::default());
        assert!(!report.has_regression);
        assert!(report.has_drift);
    }

    #[test]
    fn type_change_is_a_regression_and_suppresses_stat_findings() {
        let baseline = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let candidate = schema(vec![column("age", ColumnType::String, None)]);
        let report = compare_contracts(&baseline, &candidate, &DriftThresholds::default());
        assert!(report.has_regression);
        assert_eq!(report.findings.len(), 1);
    }

    #[test]
    fn mean_beyond_threshold_is_drift_not_regression() {
        let baseline = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let candidate = schema(vec![column("age", ColumnType::Integer, Some(60.0))]);
        let report = compare_contracts(&baseline, &candidate, &DriftThresholds::default());
        assert!(!report.has_regression);
        assert!(report.has_drift);
        assert!(matches!(report.findings[0], DriftFinding::MeanDrift { .. }));
    }

    #[test]
    fn mean_within_threshold_is_not_drift() {
        let baseline = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let candidate = schema(vec![column("age", ColumnType::Integer, Some(31.0))]);
        let report = compare_contracts(&baseline, &candidate, &DriftThresholds::default());
        assert!(!report.has_drift);
    }

    #[test]
    fn zero_baseline_mean_is_skipped_not_a_division_by_zero_panic() {
        let baseline = schema(vec![column("balance", ColumnType::Integer, Some(0.0))]);
        let candidate = schema(vec![column("balance", ColumnType::Integer, Some(100.0))]);
        let report = compare_contracts(&baseline, &candidate, &DriftThresholds::default());
        assert!(!report.has_drift);
    }

    #[test]
    fn risk_degraded_when_k_anonymity_drops() {
        let baseline_schema = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let candidate_schema = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let report = compare_with_risk(
            &baseline_schema,
            Some(&RiskAssessment::KAnonymity(10)),
            &candidate_schema,
            Some(&RiskAssessment::KAnonymity(2)),
            &DriftThresholds::default(),
        );
        assert!(report.has_drift);
        assert!(!report.has_regression);
        assert_eq!(
            report.findings,
            vec![DriftFinding::RiskDegraded {
                model: "k_anonymity".to_string(),
                baseline: 10.0,
                candidate: 2.0,
            }]
        );
    }

    #[test]
    fn risk_not_degraded_when_k_anonymity_improves() {
        let s = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let report = compare_with_risk(
            &s,
            Some(&RiskAssessment::KAnonymity(2)),
            &s,
            Some(&RiskAssessment::KAnonymity(10)),
            &DriftThresholds::default(),
        );
        assert!(!report.has_drift);
    }

    #[test]
    fn mismatched_risk_models_are_not_compared() {
        let s = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let report = compare_with_risk(
            &s,
            Some(&RiskAssessment::KAnonymity(2)),
            &s,
            Some(&RiskAssessment::TCloseness(0.9)),
            &DriftThresholds::default(),
        );
        assert!(!report.has_drift);
    }

    #[test]
    fn threshold_overrides_fall_back_to_defaults_when_unset() {
        let t = DriftThresholds::with_overrides(Some(1.0), None, None);
        let defaults = DriftThresholds::default();
        assert_eq!(t.max_null_percentage_delta, 1.0);
        assert_eq!(t.max_mean_relative_delta, defaults.max_mean_relative_delta);
        assert_eq!(
            t.max_type_consistency_delta,
            defaults.max_type_consistency_delta
        );
    }

    #[test]
    fn drift_profile_round_trips_through_json() {
        let profile = DriftProfile {
            schema: schema(vec![column("age", ColumnType::Integer, Some(30.0))]),
            risk_assessment: Some(RiskAssessment::KAnonymity(5)),
        };
        let json = serde_json::to_string(&profile).unwrap();
        let path = std::env::temp_dir().join("driftguard_profile_round_trip_test.json");
        std::fs::write(&path, &json).unwrap();
        let loaded = load_profile(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(loaded.schema.columns.len(), 1);
        assert_eq!(loaded.risk_assessment, Some(RiskAssessment::KAnonymity(5)));
    }

    #[test]
    fn load_profile_also_accepts_a_bare_schema_contract_json() {
        let bare_schema = schema(vec![column("age", ColumnType::Integer, Some(30.0))]);
        let json = serde_json::to_string(&bare_schema).unwrap();
        let path = std::env::temp_dir().join("driftguard_bare_schema_test.json");
        std::fs::write(&path, &json).unwrap();
        let loaded = load_profile(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(loaded.schema.columns.len(), 1);
        assert_eq!(loaded.risk_assessment, None);
    }

    #[test]
    fn compare_paths_detects_drift_between_two_fixture_files() {
        let report = compare_paths(
            Path::new("../../tests/fixtures/driftguard.baseline.csv"),
            Path::new("../../tests/fixtures/driftguard.candidate.csv"),
            1000,
            &DriftThresholds::default(),
        )
        .unwrap();
        assert!(report.has_drift);
    }
}
