//! Edge cases, privacy/wording guards and output-contract checks (the design notes
//! modelcard.md sections 2-4).

use serde_json::Value;
use triplex_io::Sample;
use triplex_modelcard::{
    evaluate, render_html, render_python_script, ModelCardReport, Status, DISCLAIMER,
};
use triplex_schemacast::{parse_contract, DeclaredModelCard};

fn config(block: &str) -> DeclaredModelCard {
    let yaml = format!(
        "version: 1\ncolumns: []\nmodelcard:\n{}",
        block
            .lines()
            .map(|l| format!("  {l}\n"))
            .collect::<String>()
    );
    parse_contract(&yaml).unwrap().modelcard.unwrap()
}

fn sample(headers: &[&str], rows: &[&[&str]]) -> Sample {
    Sample {
        headers: headers.iter().map(|h| h.to_string()).collect(),
        rows: rows
            .iter()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect(),
    }
}

/// `count` rows for `group`: `positives` actual positives, all predicted correctly.
fn group_rows(group: &str, count: usize, positives: usize) -> Vec<Vec<String>> {
    (0..count)
        .map(|i| {
            let y = if i < positives { "1" } else { "0" };
            vec![y.into(), y.into(), group.into()]
        })
        .collect()
}

fn table(groups: &[(&str, usize, usize)]) -> Sample {
    Sample {
        headers: vec!["label".into(), "prediction".into(), "g".into()],
        rows: groups
            .iter()
            .flat_map(|(g, n, p)| group_rows(g, *n, *p))
            .collect(),
    }
}

const BASE: &str = "target_column: label\nprediction_column: prediction\nslice_by: [g]\nprotected_attribute: g\nmin_slice_size: 30";

fn run(sample: &Sample, block: &str) -> ModelCardReport {
    evaluate(sample, &config(block)).unwrap()
}

// ---- edge cases -------------------------------------------------------------------------

#[test]
fn slice_size_boundary_is_exact_at_29_30_31() {
    for (n, suppressed) in [(29, true), (30, false), (31, false)] {
        let r = run(&table(&[("a", n, n / 2), ("b", 40, 20)]), BASE);
        let a = r.slices[0].groups.iter().find(|g| g.group == "a").unwrap();
        assert_eq!(a.suppressed, suppressed, "n={n}");
        assert_eq!(
            a.metrics.is_none(),
            suppressed,
            "n={n}: a suppressed slice exposes no metric"
        );
        assert_eq!(a.n, n, "the count is always reported");
    }
}

#[test]
fn a_group_with_no_positives_has_undefined_recall_and_the_card_warns_not_fails() {
    // group a has actual positives; group b has none, so b's true-positive rate is undefined.
    let r = run(&table(&[("a", 40, 20), ("b", 40, 0)]), BASE);
    let b = r.slices[0].groups.iter().find(|g| g.group == "b").unwrap();
    assert_eq!(b.metrics.as_ref().unwrap().recall, None, "undefined, not 0");
    assert_eq!(r.fairness.as_ref().unwrap().metrics.tpr_ratio, None);
}

#[test]
fn a_single_group_protected_attribute_is_a_warning_with_undefined_fairness() {
    let r = run(&table(&[("only", 60, 30)]), BASE);
    let f = r.fairness.as_ref().unwrap();
    assert_eq!(f.metrics.disparate_impact_ratio, None);
    assert_eq!(r.status, Status::Warn);
    assert_eq!(r.status.exit_code(), 1);
    assert!(r
        .warnings
        .iter()
        .any(|w| w.contains("fewer than two comparable groups")));
}

#[test]
fn null_slice_values_form_their_own_labelled_group() {
    let mut s = table(&[("a", 40, 20)]);
    s.rows.extend(group_rows("", 40, 20));
    let r = run(&s, BASE);
    assert!(r.slices[0]
        .groups
        .iter()
        .any(|g| g.group == "(null)" && g.n == 40));
}

#[test]
fn label_encodings_true_false_and_yes_no_resolve_through_positive_label() {
    let block = "target_column: label\nprediction_column: prediction\npositive_label: \"True\"\nmin_slice_size: 1";
    let s = sample(
        &["label", "prediction"],
        &[
            &["true", "TRUE"],
            &["false", "false"],
            &["True", "false"],
            &["false", "true"],
        ],
    );
    let r = run(&s, block);
    assert_eq!(r.overall.accuracy, 0.5);
    assert_eq!(r.overall.base_rate, 0.5);
    let yes = "target_column: label\nprediction_column: prediction\npositive_label: yes\nmin_slice_size: 1";
    let s = sample(&["label", "prediction"], &[&["Yes", "yes"], &["no", "no"]]);
    assert_eq!(run(&s, yes).overall.accuracy, 1.0);
}

#[test]
fn more_than_two_label_values_is_a_clear_error() {
    let s = sample(&["label", "prediction"], &[&["a", "b"], &["c", "a"]]);
    let e = evaluate(
        &s,
        &config("target_column: label\nprediction_column: prediction"),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("binary classification only"), "{e}");
}

#[test]
fn a_positive_label_absent_from_the_data_is_a_clear_error() {
    let s = sample(&["label", "prediction"], &[&["a", "b"], &["b", "a"]]);
    let e = evaluate(
        &s,
        &config("target_column: label\nprediction_column: prediction\npositive_label: z"),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("positive_label"), "{e}");
}

#[test]
fn a_missing_column_is_a_clear_error() {
    let e = evaluate(
        &table(&[("a", 5, 2)]),
        &config("target_column: nope\nprediction_column: prediction"),
    )
    .unwrap_err();
    assert_eq!(e.code(), "COLUMN_NOT_FOUND");
}

#[test]
fn rows_with_an_empty_label_or_prediction_are_dropped_and_reported() {
    let mut s = table(&[("a", 40, 20), ("b", 40, 20)]);
    s.rows.push(vec!["".into(), "1".into(), "a".into()]);
    s.rows.push(vec!["1".into(), " ".into(), "b".into()]);
    let r = run(&s, BASE);
    assert_eq!(
        (
            r.data.rows_read,
            r.data.rows_used,
            r.data.rows_dropped_missing_label
        ),
        (82, 80, 2)
    );
    assert!(r.warnings.iter().any(|w| w.contains("2 row(s) dropped")));
}

#[test]
fn a_bad_score_is_an_error_naming_the_row_never_the_value() {
    let s = sample(
        &["label", "prediction", "score"],
        &[&["1", "1", "0.9"], &["0", "0", "secret-7"]],
    );
    let e = evaluate(
        &s,
        &config("target_column: label\nprediction_column: prediction\nscore_column: score"),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("data row 2"), "{e}");
    assert!(
        !e.contains("secret-7"),
        "the offending value must not appear: {e}"
    );
    let nan = sample(&["label", "prediction", "score"], &[&["1", "1", "NaN"]]);
    assert!(evaluate(
        &nan,
        &config("target_column: label\nprediction_column: prediction\nscore_column: score")
    )
    .is_err());
}

#[test]
fn a_three_decimal_score_is_read_as_a_probability_not_a_thousands_grouped_integer() {
    // 0.639 must stay 0.639 (SchemaCast's locale normalizer would read it as 639).
    let s = sample(
        &["label", "prediction", "score"],
        &[&["0", "0", "0.639"], &["1", "1", "0.641"]],
    );
    let r = run(&s, "target_column: label\nprediction_column: prediction\nscore_column: score\nmin_slice_size: 1");
    assert_eq!(r.overall.roc_auc, Some(1.0));
}

#[test]
fn a_nan_threshold_fails_closed() {
    let r = run(
        &table(&[("a", 40, 20), ("b", 40, 20)]),
        &format!("{BASE}\nthresholds:\n  disparate_impact_min: .nan"),
    );
    assert_eq!(
        r.status,
        Status::Fail,
        "a NaN threshold must reject, never silently pass"
    );
}

// ---- safety -----------------------------------------------------------------------------

#[test]
fn a_unique_per_row_slice_column_is_rejected_not_printed_row_by_row() {
    let rows: Vec<Vec<String>> = (0..200)
        .map(|i| vec!["1".into(), "1".into(), format!("customer-{i}")])
        .collect();
    let s = Sample {
        headers: vec!["label".into(), "prediction".into(), "g".into()],
        rows,
    };
    let e = evaluate(&s, &config(BASE)).unwrap_err().to_string();
    assert!(e.contains("looks like an identifier"), "{e}");
    assert!(!e.contains("customer-1"), "no cell value in the error: {e}");
}

#[test]
fn suppressed_slices_expose_only_their_count_in_every_output() {
    let r = run(
        &table(&[("tiny", 5, 2), ("big", 60, 30), ("big2", 60, 30)]),
        BASE,
    );
    let json = serde_json::to_value(&r).unwrap();
    let tiny = json["slices"][0]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["group"] == "tiny")
        .unwrap()
        .clone();
    assert_eq!(tiny["suppressed"], true);
    assert_eq!(tiny["metrics"], Value::Null);
    assert!(render_html(&r).contains("suppressed"));
}

const FORBIDDEN: [&str; 5] = [
    "complies",
    "compliant",
    "certified",
    "guarantee",
    "meets the requirements",
];

#[test]
fn no_output_claims_compliance_and_the_disclaimer_is_present() {
    let r = run(
        &table(&[("a", 40, 20), ("b", 40, 20)]),
        &format!("{BASE}\nmodel:\n  name: m\n  intended_use: demo"),
    );
    for (kind, text) in [
        ("json", serde_json::to_string(&r).unwrap()),
        ("html", render_html(&r)),
    ] {
        let lowered = text.to_lowercase();
        for word in FORBIDDEN {
            assert!(
                !lowered.contains(word),
                "{kind} output contains the forbidden claim word {word:?}"
            );
        }
        assert!(
            text.contains(&DISCLAIMER[..40]),
            "{kind} output must carry the disclaimer"
        );
    }
}

#[test]
fn hostile_group_names_are_html_escaped_and_the_report_has_no_external_urls() {
    let r = run(
        &table(&[("<script>alert(1)</script>", 40, 20), ("b", 40, 20)]),
        BASE,
    );
    let html = render_html(&r);
    assert!(
        !html.contains("<script>alert"),
        "group name must be escaped"
    );
    assert!(html.contains("&lt;script&gt;"));
    assert!(
        !html.contains("http://") && !html.contains("https://"),
        "report must be self-contained"
    );
}

// ---- output contract --------------------------------------------------------------------

#[test]
fn the_card_is_byte_identical_across_repeated_runs_and_carries_a_version() {
    let s = table(&[("a", 40, 20), ("b", 40, 15), ("c", 12, 6)]);
    let first = serde_json::to_string_pretty(&run(&s, BASE)).unwrap();
    let second = serde_json::to_string_pretty(&run(&s, BASE)).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("\"version\": \"0.1.0\""));
}

#[test]
fn a_passing_card_exits_zero_and_a_failing_one_exits_two() {
    let even = run(&table(&[("a", 40, 20), ("b", 40, 20)]), BASE);
    assert_eq!((even.status, even.status.exit_code()), (Status::Pass, 0));
    let skewed = run(&table(&[("a", 40, 30), ("b", 40, 8)]), BASE);
    assert_eq!(
        (skewed.status, skewed.status.exit_code()),
        (Status::Fail, 2)
    );
}

/// Runs `reproduce.py` with real Polars and compares it to the Rust engine on the oracle fixture.
/// Skipped (with a message) when Polars is not importable. Set TRIPLEX_PYTHON to a Python that has
/// it; CI without Polars skips this, so run it locally before releasing.
#[test]
fn reproduce_py_matches_the_rust_engine() {
    let python = std::env::var("TRIPLEX_PYTHON").unwrap_or_else(|_| "python3".into());
    let has_polars = std::process::Command::new(&python)
        .args(["-c", "import polars"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_polars {
        eprintln!("SKIPPED reproduce_py_matches_the_rust_engine: {python} has no polars (set TRIPLEX_PYTHON)");
        return;
    }
    let dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/modelcard");
    let contract = triplex_schemacast::load_contract(&dir.join("eval.contract.yaml")).unwrap();
    let cfg = contract.modelcard.as_ref().unwrap();
    let input = dir.join("eval.csv");
    let engine =
        serde_json::to_value(triplex_modelcard::evaluate_path(&input, cfg).unwrap()).unwrap();

    let script_path = std::env::temp_dir().join("triplex_modelcard_reproduce_test.py");
    std::fs::write(
        &script_path,
        render_python_script(input.to_str().unwrap(), cfg),
    )
    .unwrap();
    let out = std::process::Command::new(&python)
        .arg(&script_path)
        .output()
        .unwrap();
    std::fs::remove_file(&script_path).ok();
    assert!(
        out.status.success(),
        "reproduce.py failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let script: Value = serde_json::from_slice(&out.stdout).unwrap();

    let close = |what: &str, a: &Value, b: &Value| match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => assert!((x - y).abs() < 1e-9, "{what}: rust {x} vs script {y}"),
        (None, None) => {}
        _ => panic!("{what}: rust {a} vs script {b}"),
    };
    for key in [
        "selection_rate",
        "base_rate",
        "accuracy",
        "precision",
        "recall",
        "fpr",
        "f1",
        "roc_auc",
    ] {
        close(
            &format!("overall.{key}"),
            &engine["overall"][key],
            &script["overall"][key],
        );
    }
    let mut groups = 0;
    for column in engine["slices"].as_array().unwrap() {
        let name = column["column"].as_str().unwrap();
        for group in column["groups"].as_array().unwrap() {
            let g = group["group"].as_str().unwrap();
            let theirs = &script["slices"][name][g];
            assert_eq!(group["n"], theirs["n"], "{name}={g} n");
            assert_eq!(
                group["suppressed"], theirs["suppressed"],
                "{name}={g} suppressed"
            );
            if group["suppressed"] == false {
                for key in [
                    "selection_rate",
                    "accuracy",
                    "precision",
                    "recall",
                    "fpr",
                    "f1",
                    "roc_auc",
                ] {
                    close(
                        &format!("{name}={g}.{key}"),
                        &group["metrics"][key],
                        &theirs[key],
                    );
                }
                groups += 1;
            }
        }
    }
    assert!(groups >= 5);
    for key in [
        "demographic_parity_difference",
        "disparate_impact_ratio",
        "tpr_ratio",
        "fpr_ratio",
        "equalized_odds_difference",
        "equalized_odds_ratio",
    ] {
        close(
            &format!("fairness.{key}"),
            &engine["fairness"]["metrics"][key],
            &script["fairness"][key],
        );
    }
}

// ---- gaps found by mutation testing (cargo-mutants, 2026-09-19) --------------------------

#[test]
fn a_max_threshold_gates_the_parity_difference_in_both_directions() {
    // selection rates 0.75 vs 0.20 -> difference 0.55.
    let s = table(&[("a", 40, 30), ("b", 40, 8)]);
    let loose = "thresholds:\n  disparate_impact_min: 0.0\n  equalized_odds_min: 0.0\n  demographic_parity_difference_max: 0.60";
    let tight = "thresholds:\n  disparate_impact_min: 0.0\n  equalized_odds_min: 0.0\n  demographic_parity_difference_max: 0.50";
    assert_eq!(
        run(&s, &format!("{BASE}\n{loose}")).status,
        Status::Pass,
        "0.55 is within a 0.60 maximum"
    );
    assert_eq!(
        run(&s, &format!("{BASE}\n{tight}")).status,
        Status::Fail,
        "0.55 exceeds a 0.50 maximum"
    );
}

#[test]
fn the_group_cap_allows_exactly_the_cap_and_rejects_one_more() {
    let groups = |n: usize| -> Sample {
        Sample {
            headers: vec!["label".into(), "prediction".into(), "g".into()],
            rows: (0..n)
                .flat_map(|i| group_rows(&format!("g{i}"), 2, 1))
                .collect(),
        }
    };
    let block = "target_column: label\nprediction_column: prediction\nslice_by: [g]\nmax_groups_per_column: 5\nmin_slice_size: 1";
    assert!(
        evaluate(&groups(5), &config(block)).is_ok(),
        "exactly the cap is allowed"
    );
    assert!(
        evaluate(&groups(6), &config(block)).is_err(),
        "one over the cap is rejected"
    );
}

#[test]
fn an_undefined_fairness_metric_alone_is_a_warning() {
    // a is all positives (no false-positive rate), b is all negatives (no true-positive rate):
    // equalized odds is undefined although no slice is suppressed and no row is dropped.
    let s = table(&[("a", 40, 40), ("b", 40, 0)]);
    let r = run(
        &s,
        &format!("{BASE}\nthresholds:\n  disparate_impact_min: 0.0"),
    );
    assert!(
        r.warnings.is_empty(),
        "nothing else warns: {:?}",
        r.warnings
    );
    assert_eq!(
        r.fairness.as_ref().unwrap().metrics.equalized_odds_ratio,
        None
    );
    assert_eq!(r.status, Status::Warn);
}
