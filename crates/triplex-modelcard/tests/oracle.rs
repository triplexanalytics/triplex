//! Compares the engine against golden values produced by independent oracles (scikit-learn for
//! per-slice metrics, Fairlearn for fairness) by `scripts/modelcard_oracle.py`. No Python is
//! needed to run this test: the goldens are checked in.

use serde_json::Value;
use std::path::PathBuf;
use triplex_modelcard::{config_from_contract, evaluate_path, ModelCardReport};

const TOLERANCE: f64 = 1e-9;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/modelcard")
        .join(name)
}

fn report() -> ModelCardReport {
    let contract = triplex_schemacast::load_contract(&fixture("eval.contract.yaml")).unwrap();
    evaluate_path(
        &fixture("eval.csv"),
        config_from_contract(&contract).unwrap(),
    )
    .unwrap()
}

fn golden() -> Value {
    serde_json::from_str(&std::fs::read_to_string(fixture("eval.golden.json")).unwrap()).unwrap()
}

fn assert_close(what: &str, actual: &Value, expected: &Value) {
    match (actual.as_f64(), expected.as_f64()) {
        (Some(a), Some(e)) => assert!(
            (a - e).abs() <= TOLERANCE,
            "{what}: engine {a} vs oracle {e}"
        ),
        (None, None) => assert!(
            actual.is_null() && expected.is_null(),
            "{what}: engine {actual} vs oracle {expected}"
        ),
        _ => panic!("{what}: engine {actual} vs oracle {expected}"),
    }
}

const METRIC_KEYS: [&str; 8] = [
    "selection_rate",
    "base_rate",
    "accuracy",
    "precision",
    "recall",
    "fpr",
    "f1",
    "roc_auc",
];

fn compare_metrics(what: &str, engine: &Value, oracle: &Value) {
    for key in METRIC_KEYS {
        assert_close(&format!("{what}.{key}"), &engine[key], &oracle[key]);
    }
}

#[test]
fn overall_and_every_slice_metric_matches_scikit_learn() {
    let (engine, oracle) = (serde_json::to_value(report()).unwrap(), golden());
    compare_metrics("overall", &engine["overall"], &oracle["overall"]);
    assert_eq!(
        engine["data"]["rows_used"].as_u64(),
        oracle["overall"]["n"].as_u64()
    );

    let mut compared = 0;
    for column in engine["slices"].as_array().unwrap() {
        let name = column["column"].as_str().unwrap();
        for group in column["groups"].as_array().unwrap() {
            let key = group["group"].as_str().unwrap();
            let expected = &oracle["slices"][name][key];
            assert_eq!(group["n"], expected["n"], "n for {name}={key}");
            if expected["suppressed"].as_bool() == Some(true) {
                assert_eq!(group["suppressed"], true, "{name}={key} must be suppressed");
                assert!(
                    group["metrics"].is_null(),
                    "{name}={key} must expose no metrics"
                );
            } else {
                compare_metrics(&format!("{name}={key}"), &group["metrics"], expected);
                compared += 1;
            }
        }
    }
    assert!(
        compared >= 5,
        "expected several slices to be compared, got {compared}"
    );
}

#[test]
fn fairness_matches_fairlearn_with_the_small_group_left_out() {
    let (engine, oracle) = (serde_json::to_value(report()).unwrap(), golden());
    let (f, g) = (&engine["fairness"]["metrics"], &oracle["fairness"]);
    assert_close(
        "demographic_parity_difference",
        &f["demographic_parity_difference"],
        &g["demographic_parity_difference"],
    );
    assert_close(
        "disparate_impact_ratio (= demographic_parity_ratio)",
        &f["disparate_impact_ratio"],
        &g["demographic_parity_ratio"],
    );
    assert_close(
        "equalized_odds_difference",
        &f["equalized_odds_difference"],
        &g["equalized_odds_difference"],
    );
    assert_close(
        "equalized_odds_ratio",
        &f["equalized_odds_ratio"],
        &g["equalized_odds_ratio"],
    );

    let compared: Vec<&str> = engine["fairness"]["groups_compared"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let expected: Vec<&str> = g["groups_compared"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(compared, expected);
    assert_eq!(
        engine["fairness"]["groups_suppressed"],
        serde_json::json!(["islands"])
    );
}

#[test]
fn the_injected_bias_is_caught_as_a_fairness_failure() {
    // south's true-positive rate (0.65) trails east's (0.91): equalized odds ratio ~0.30, far
    // below the 0.85 default, so the card must fail (exit code 2), not merely warn.
    let r = report();
    assert_eq!(r.status.exit_code(), 2);
    let failing: Vec<&str> = r
        .fairness
        .as_ref()
        .unwrap()
        .checks
        .iter()
        .filter(|c| c.outcome == triplex_modelcard::CheckOutcome::Fail)
        .map(|c| c.name.as_str())
        .collect();
    assert!(
        failing.contains(&"equalized_odds_ratio"),
        "failing checks: {failing:?}"
    );
}
