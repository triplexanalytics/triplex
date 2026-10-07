//! Tier 2 synthesis (T2.1) evaluation: determinism, fidelity, identifier handling, rare-category
//! suppression, leakage measurement, wording, and parity with the generated reproduce.py.

use serde_json::Value;
use triplex_anonymizer::synthesize::{
    render_html, render_python_script, synthesize, SynthesisResult, SynthesizeConfig,
};
use triplex_io::Sample;

/// Test-local SplitMix64 so the fixture does not depend on the crate's own generator.
struct Gen(u64);
impl Gen {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 0.5) / 9_007_199_254_740_992.0
    }
    fn normal(&mut self) -> f64 {
        (-2.0 * self.uniform().ln()).sqrt() * (2.0 * std::f64::consts::PI * self.uniform()).cos()
    }
}

/// age ~ N(45,12); income correlated with age (about 0.7); score independent; `plan` depends on
/// income; `region` independent; 10% of `income` is null; two rows carry the rare category "vip".
fn table(n: usize) -> Sample {
    let mut g = Gen(2024);
    let headers = [
        "customer_id",
        "email",
        "note",
        "age",
        "income",
        "score",
        "plan",
        "region",
    ];
    let rows = (0..n)
        .map(|i| {
            let age = (45.0 + 12.0 * g.normal()).clamp(18.0, 90.0).round();
            let income = 20_000.0 + 900.0 * age + 9_000.0 * g.normal();
            let score = (0.5 + 0.2 * g.normal()).clamp(0.0, 1.0);
            let plan = if i < 2 {
                "vip"
            } else if income > 65_000.0 {
                "pro"
            } else if income > 50_000.0 {
                "plus"
            } else {
                "free"
            };
            let region = ["north", "south", "east", "west"][(g.next() % 4) as usize];
            vec![
                (1000 + i).to_string(),
                format!("user{i}@example.com"),
                format!("free text {i} {}", g.next()),
                format!("{age}"),
                if g.uniform() < 0.1 {
                    String::new()
                } else {
                    format!("{income:.2}")
                },
                format!("{score:.4}"),
                plan.to_string(),
                region.to_string(),
            ]
        })
        .collect();
    Sample {
        headers: headers.iter().map(|h| h.to_string()).collect(),
        rows,
    }
}

fn run(n: usize, config: &SynthesizeConfig) -> SynthesisResult {
    synthesize(&table(n), config).unwrap()
}

fn column<'a>(r: &'a SynthesisResult, name: &str) -> Vec<&'a str> {
    let i = r.sample.headers.iter().position(|h| h == name).unwrap();
    r.sample.rows.iter().map(|row| row[i].as_str()).collect()
}

#[test]
fn the_same_seed_gives_identical_output_and_a_different_seed_does_not() {
    let a = run(
        400,
        &SynthesizeConfig {
            seed: 5,
            ..Default::default()
        },
    );
    let b = run(
        400,
        &SynthesizeConfig {
            seed: 5,
            ..Default::default()
        },
    );
    let c = run(
        400,
        &SynthesizeConfig {
            seed: 6,
            ..Default::default()
        },
    );
    assert_eq!(a.sample.rows, b.sample.rows);
    assert_ne!(a.sample.rows, c.sample.rows);
    assert_eq!(
        serde_json::to_string(&a.report).unwrap(),
        serde_json::to_string(&b.report).unwrap()
    );
}

#[test]
fn the_row_count_defaults_to_the_input_and_can_be_overridden() {
    assert_eq!(
        run(300, &SynthesizeConfig::default()).sample.rows.len(),
        300
    );
    assert_eq!(
        run(
            300,
            &SynthesizeConfig {
                rows: Some(50),
                ..Default::default()
            }
        )
        .sample
        .rows
        .len(),
        50
    );
    assert_eq!(
        run(
            300,
            &SynthesizeConfig {
                rows: Some(1000),
                ..Default::default()
            }
        )
        .sample
        .rows
        .len(),
        1000
    );
}

#[test]
fn identifier_like_columns_are_dropped_with_a_reason_and_never_synthesized() {
    let r = run(300, &SynthesizeConfig::default());
    let dropped: Vec<&str> = r
        .report
        .columns_dropped
        .iter()
        .map(|d| d.column.as_str())
        .collect();
    for expected in ["customer_id", "email", "note"] {
        assert!(
            dropped.contains(&expected),
            "{expected} should be dropped, dropped: {dropped:?}"
        );
        assert!(
            !r.sample.headers.contains(&expected.to_string()),
            "{expected} must not appear in the output"
        );
    }
    assert!(r
        .report
        .columns_dropped
        .iter()
        .all(|d| !d.reason.is_empty()));
    assert_eq!(
        r.report.columns_synthesized,
        vec!["age", "income", "score", "plan", "region"]
    );
}

#[test]
fn a_rare_category_is_merged_and_never_reproduced() {
    let r = run(500, &SynthesizeConfig::default());
    assert!(
        !column(&r, "plan").contains(&"vip"),
        "a category seen twice must not appear"
    );
    let merged = r
        .report
        .categories_merged
        .iter()
        .find(|m| m.column == "plan")
        .unwrap();
    assert_eq!((merged.categories_merged, merged.rows_affected), (1, 2));
    assert!(
        column(&r, "plan").contains(&"(other)"),
        "the merged bucket is used instead"
    );
}

#[test]
fn marginals_and_correlation_are_preserved() {
    let r = run(4000, &SynthesizeConfig::default());
    for c in &r.report.fidelity.columns {
        match c.kind {
            "numeric" => assert!(
                c.ks_statistic.unwrap() < 0.06,
                "{}: KS {:?}",
                c.column,
                c.ks_statistic
            ),
            _ => assert!(
                c.total_variation.unwrap() < 0.06,
                "{}: TV {:?}",
                c.column,
                c.total_variation
            ),
        }
    }
    let corr = &r.report.fidelity.correlation;
    assert!(corr.pairs_compared >= 3);
    assert!(
        corr.max_abs_error.unwrap() < 0.1,
        "max correlation error {:?}",
        corr.max_abs_error
    );
}

#[test]
fn the_age_income_dependence_survives_synthesis() {
    let r = run(4000, &SynthesizeConfig::default());
    let (age, income) = (column(&r, "age"), column(&r, "income"));
    let pairs: Vec<(f64, f64)> = age
        .iter()
        .zip(&income)
        .filter(|(_, i)| !i.is_empty())
        .map(|(a, i)| (a.parse().unwrap(), i.parse().unwrap()))
        .collect();
    let n = pairs.len() as f64;
    let (ma, mi) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let cov: f64 = pairs.iter().map(|p| (p.0 - ma) * (p.1 - mi)).sum();
    let (va, vi): (f64, f64) = (
        pairs.iter().map(|p| (p.0 - ma).powi(2)).sum(),
        pairs.iter().map(|p| (p.1 - mi).powi(2)).sum(),
    );
    let r = cov / (va.sqrt() * vi.sqrt());
    assert!(
        (0.55..0.85).contains(&r),
        "the real age-income correlation is about 0.7, synthetic gave {r}"
    );
}

#[test]
fn null_rates_are_preserved() {
    let r = run(4000, &SynthesizeConfig::default());
    let income = r
        .report
        .fidelity
        .columns
        .iter()
        .find(|c| c.column == "income")
        .unwrap();
    assert!(
        (income.null_rate_real - income.null_rate_synthetic).abs() < 0.02,
        "{income:?}"
    );
    assert!(income.null_rate_real > 0.05);
}

#[test]
fn continuous_synthetic_rows_are_not_copies_of_real_rows() {
    let r = run(2000, &SynthesizeConfig::default());
    let leak = &r.report.leakage;
    assert_eq!(
        leak.exact_copy_rows, 0,
        "no synthetic row should equal a real row when income and score are continuous"
    );
    // synthetic rows sit about as far from the real table as real rows sit from each other
    let ratio = leak.dcr_ratio.unwrap();
    assert!((0.7..1.5).contains(&ratio), "nearest-record ratio {ratio}");
}

#[test]
fn the_report_makes_no_privacy_or_dp_claim_and_says_so() {
    let r = run(300, &SynthesizeConfig::default());
    let json = serde_json::to_string(&r.report).unwrap().to_lowercase();
    let html = render_html(&r.report).to_lowercase();
    for (kind, text) in [("json", json), ("html", html)] {
        assert!(
            text.contains("no formal privacy guarantee"),
            "{kind} must carry the statement"
        );
        for claim in [
            "differentially private",
            "epsilon",
            "guaranteed",
            "safe to share",
            "fully anonymous",
        ] {
            assert!(!text.contains(claim), "{kind} contains the claim {claim:?}");
        }
    }
}

#[test]
fn the_html_report_escapes_column_names_and_has_no_external_urls() {
    let mut sample = table(100);
    sample.headers[6] = "<b>plan</b>".to_string();
    let html = render_html(
        &synthesize(&sample, &SynthesizeConfig::default())
            .unwrap()
            .report,
    );
    assert!(!html.contains("<b>plan</b>") && html.contains("&lt;b&gt;plan"));
    assert!(!html.contains("http://") && !html.contains("https://"));
}

#[test]
fn unusable_input_is_a_clear_error() {
    let tiny = Sample {
        headers: vec!["a".into()],
        rows: (0..5).map(|i| vec![i.to_string()]).collect(),
    };
    assert!(synthesize(&tiny, &SynthesizeConfig::default())
        .err()
        .unwrap()
        .to_string()
        .contains("at least"));
    let ids_only = Sample {
        headers: vec!["id".into(), "email".into()],
        rows: (0..40)
            .map(|i| vec![i.to_string(), format!("u{i}@example.com")])
            .collect(),
    };
    assert!(synthesize(&ids_only, &SynthesizeConfig::default())
        .err()
        .unwrap()
        .to_string()
        .contains("no column can be synthesized"));
}

#[test]
fn synthesize_file_writes_a_csv_that_reads_back() {
    let dir = std::env::temp_dir().join("triplex_synth_file_test");
    std::fs::create_dir_all(&dir).unwrap();
    let (input, output) = (dir.join("in.csv"), dir.join("out.csv"));
    let sample = table(200);
    triplex_io::write_csv(&input, &sample.headers, &sample.rows).unwrap();
    let report = triplex_anonymizer::synthesize::synthesize_file(
        &input,
        &output,
        &SynthesizeConfig::default(),
    )
    .unwrap();
    let back = triplex_io::read_csv_full(&output).unwrap();
    assert_eq!(back.headers, report.columns_synthesized);
    assert_eq!(back.rows.len(), 200);
    std::fs::remove_dir_all(&dir).ok();
}

/// Runs reproduce.py with real Polars and compares its fidelity numbers with the Rust report, for
/// every non-private method: the script re-derives the evaluation from the two files, so it does
/// not depend on which model wrote the synthetic one.
/// Skipped when Polars is not importable (set TRIPLEX_PYTHON); CI without it skips this.
#[test]
fn reproduce_py_matches_the_rust_report() {
    use triplex_anonymizer::synthesize::bayesnet::{
        synthesize_bayes_file, BayesConfig, StructureKind,
    };
    let python = std::env::var("TRIPLEX_PYTHON").unwrap_or_else(|_| "python3".into());
    if !std::process::Command::new(&python)
        .args(["-c", "import polars"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        eprintln!("SKIPPED reproduce_py_matches_the_rust_report: {python} has no polars (set TRIPLEX_PYTHON)");
        return;
    }
    let config = SynthesizeConfig::default();
    let network = |structure| BayesConfig {
        structure,
        ..BayesConfig::default()
    };
    type Run<'a> = &'a dyn Fn(
        &std::path::Path,
        &std::path::Path,
    ) -> triplex_anonymizer::synthesize::SynthesisReport;
    let sketch_of = |i: &std::path::Path| {
        triplex_anonymizer::synthesize::sketch::propose_file(i, &Default::default()).unwrap()
    };
    let methods: [(&str, Run); 5] = [
        ("copula", &|i, o| {
            triplex_anonymizer::synthesize::synthesize_file(i, o, &config).unwrap()
        }),
        ("tree", &|i, o| {
            synthesize_bayes_file(i, o, &network(StructureKind::ChowLiu)).unwrap()
        }),
        ("k_parent", &|i, o| {
            synthesize_bayes_file(i, o, &network(StructureKind::KParent)).unwrap()
        }),
        ("independent", &|i, o| {
            synthesize_bayes_file(i, o, &network(StructureKind::Independent)).unwrap()
        }),
        ("sketch", &|i, o| {
            triplex_anonymizer::synthesize::sketch::synthesize_sketch_file(
                i,
                o,
                &sketch_of(i),
                &config,
            )
            .unwrap()
        }),
    ];
    for (method, run) in methods {
        reproduce_py_parity(&python, method, run, config.min_category_count);
    }
}

fn reproduce_py_parity(
    python: &str,
    method: &str,
    run: &dyn Fn(
        &std::path::Path,
        &std::path::Path,
    ) -> triplex_anonymizer::synthesize::SynthesisReport,
    min_category_count: usize,
) {
    let dir = std::env::temp_dir().join(format!("triplex_synth_parity_test_{method}"));
    std::fs::create_dir_all(&dir).unwrap();
    let (input, output, script) = (
        dir.join("real.csv"),
        dir.join("synthetic.csv"),
        dir.join("reproduce.py"),
    );
    let real = table(1500);
    triplex_io::write_csv(&input, &real.headers, &real.rows).unwrap();
    let report = run(&input, &output);
    std::fs::write(
        &script,
        render_python_script(
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            min_category_count,
        ),
    )
    .unwrap();
    let out = std::process::Command::new(python)
        .arg(&script)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "reproduce.py failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let theirs: Value = serde_json::from_slice(&out.stdout).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    let close = |what: &str, a: Option<f64>, b: &Value| match (a, b.as_f64()) {
        (Some(x), Some(y)) => assert!(
            (x - y).abs() < 1e-9,
            "{method} {what}: rust {x} vs script {y}"
        ),
        (None, None) => {}
        other => panic!("{method} {what}: {other:?}"),
    };
    for c in &report.fidelity.columns {
        let t = &theirs["columns"][&c.column];
        close(
            &format!("{} null_rate_real", c.column),
            Some(c.null_rate_real),
            &t["null_rate_real"],
        );
        close(
            &format!("{} null_rate_synthetic", c.column),
            Some(c.null_rate_synthetic),
            &t["null_rate_synthetic"],
        );
        close(
            &format!("{} ks", c.column),
            c.ks_statistic,
            &t["ks_statistic"],
        );
        close(
            &format!("{} tvd", c.column),
            c.total_variation,
            &t["total_variation"],
        );
    }
    let corr = &report.fidelity.correlation;
    assert_eq!(
        corr.pairs_compared as u64,
        theirs["correlation"]["pairs_compared"].as_u64().unwrap()
    );
    close(
        "correlation max",
        corr.max_abs_error,
        &theirs["correlation"]["max_abs_error"],
    );
    close(
        "correlation mean",
        corr.mean_abs_error,
        &theirs["correlation"]["mean_abs_error"],
    );
    assert_eq!(
        report.leakage.exact_copy_rows as u64,
        theirs["exact_copy_rows"].as_u64().unwrap()
    );
}

#[test]
fn a_location_named_column_is_kept_when_coarse_and_dropped_when_it_identifies() {
    let build = |distinct: usize| Sample {
        headers: vec!["city".into(), "age".into()],
        rows: (0..100)
            .map(|i| vec![format!("place-{}", i % distinct), (20 + i % 40).to_string()])
            .collect(),
    };
    let coarse = synthesize(&build(8), &SynthesizeConfig::default()).unwrap();
    assert!(
        coarse
            .report
            .columns_synthesized
            .contains(&"city".to_string()),
        "8 cities in 100 rows is a category"
    );
    let fine = synthesize(&build(60), &SynthesizeConfig::default()).unwrap();
    assert!(
        fine.report
            .columns_dropped
            .iter()
            .any(|d| d.column == "city"),
        "60 distinct in 100 rows identifies"
    );
}

#[test]
fn float_columns_are_rounded_to_the_decimals_the_real_data_has() {
    // income has 2 decimals and score 4 in the fixture; synthetic values must not carry more.
    let r = run(300, &SynthesizeConfig::default());
    let decimals = |name: &str| {
        column(&r, name)
            .iter()
            .filter(|v| !v.is_empty())
            .map(|v| v.split_once('.').map_or(0, |(_, f)| f.len()))
            .max()
            .unwrap()
    };
    assert!(
        decimals("income") <= 2,
        "income has {} decimals",
        decimals("income")
    );
    assert!(
        decimals("score") <= 4,
        "score has {} decimals",
        decimals("score")
    );
}

// ---- gaps found by mutation testing (cargo-mutants, 2026-09-20) ------------------------------

fn two_column(rows: Vec<(String, String)>) -> Sample {
    Sample {
        headers: vec!["place".into(), "code".into()],
        rows: rows.into_iter().map(|(a, b)| vec![a, b]).collect(),
    }
}

#[test]
fn the_minimum_row_count_is_exactly_ten() {
    let rows = |n: usize| Sample {
        headers: vec!["v".into()],
        rows: (0..n).map(|i| vec![(i % 3).to_string()]).collect(),
    };
    assert!(synthesize(&rows(9), &SynthesizeConfig::default()).is_err());
    assert!(synthesize(&rows(10), &SynthesizeConfig::default()).is_ok());
}

#[test]
fn a_location_column_is_dropped_at_exactly_half_unique_and_kept_just_below() {
    // 100 rows: 50 distinct values is a 0.5 unique ratio (dropped), 49 is below it (kept).
    let build = |distinct: usize| {
        two_column(
            (0..100)
                .map(|i| (format!("p{}", i % distinct), (i % 7).to_string()))
                .collect(),
        )
    };
    let named = |s: Sample| Sample {
        headers: vec!["city".into(), s.headers[1].clone()],
        rows: s.rows,
    };
    let at_half = synthesize(&named(build(50)), &SynthesizeConfig::default()).unwrap();
    assert!(
        at_half
            .report
            .columns_dropped
            .iter()
            .any(|d| d.column == "city"),
        "0.5 is identifying"
    );
    let below = synthesize(&named(build(49)), &SynthesizeConfig::default()).unwrap();
    assert!(
        below
            .report
            .columns_synthesized
            .contains(&"city".to_string()),
        "0.49 is a category"
    );
}

#[test]
fn a_category_at_exactly_the_minimum_count_is_kept_and_one_below_is_merged() {
    // 100 rows: a x46, b x45, c x5 (kept: exactly the default minimum), d x4 (merged).
    let mut values = Vec::new();
    for (name, count) in [("a", 46), ("b", 45), ("c", 5), ("d", 4)] {
        values.extend(std::iter::repeat_n(name, count));
    }
    let sample = Sample {
        headers: vec!["kind".into(), "n".into()],
        rows: values
            .iter()
            .enumerate()
            .map(|(i, v)| vec![v.to_string(), (i % 9).to_string()])
            .collect(),
    };
    let r = synthesize(
        &sample,
        &SynthesizeConfig {
            rows: Some(5000),
            ..Default::default()
        },
    )
    .unwrap();
    let kinds = column(&r, "kind");
    assert!(
        kinds.contains(&"c"),
        "a category seen exactly 5 times must survive"
    );
    assert!(
        !kinds.contains(&"d"),
        "a category seen 4 times must be merged"
    );
    let merged = &r.report.categories_merged[0];
    assert_eq!((merged.categories_merged, merged.rows_affected), (1, 4));
}

#[test]
fn a_repaired_correlation_matrix_is_reported_and_a_healthy_one_is_not() {
    let duplicated = Sample {
        headers: vec!["a".into(), "b".into()],
        rows: (0..60)
            .map(|i| vec![format!("{}.5", i), format!("{}.5", i)])
            .collect(),
    };
    let r = synthesize(&duplicated, &SynthesizeConfig::default()).unwrap();
    assert!(r.report.correlation_shrinkage > 0.0);
    assert!(r
        .report
        .warnings
        .iter()
        .any(|w| w.contains("shrunk toward the identity")));
    let healthy = run(300, &SynthesizeConfig::default());
    assert_eq!(healthy.report.correlation_shrinkage, 0.0);
    assert!(!healthy.report.warnings.iter().any(|w| w.contains("shrunk")));
}

#[test]
fn the_report_lists_its_limitations() {
    let r = run(300, &SynthesizeConfig::default());
    assert!(r.report.limitations.len() >= 4);
    assert!(r
        .report
        .limitations
        .iter()
        .any(|l| l.contains("Nulls are modelled")));
    assert!(r
        .report
        .limitations
        .iter()
        .any(|l| l.contains("Identifier-like columns")));
    assert!(
        r.report.limitations.iter().all(|l| l.len() > 20),
        "no placeholder entries"
    );
}

// ---- rows and columns the user removed from the output ----------------------------------------

#[test]
fn excluded_columns_are_never_synthesized_and_are_listed_with_a_reason() {
    let config = SynthesizeConfig {
        exclude_columns: vec!["income".into(), "age".into()],
        ..Default::default()
    };
    let r = run(300, &config);
    assert!(
        !r.sample.headers.contains(&"income".to_string())
            && !r.sample.headers.contains(&"age".to_string())
    );
    let entry = r
        .report
        .columns_dropped
        .iter()
        .find(|d| d.column == "income")
        .unwrap();
    assert_eq!(entry.reason, "removed from the output by the user");
    assert!(r.report.columns_synthesized.contains(&"plan".to_string()));
}

#[test]
fn excluded_rows_never_enter_the_fit_and_are_counted() {
    // "vip" appears only in the first two rows; with a minimum count of 1 it is reproduced unless
    // those rows are removed first.
    let sample = table(500);
    let rows_with_vip: std::collections::HashSet<usize> = [0usize, 1].into_iter().collect();
    let base = SynthesizeConfig {
        min_category_count: 1,
        rows: Some(20_000),
        ..Default::default()
    };
    let kept = synthesize(&sample, &base).unwrap();
    assert!(
        column(&kept, "plan").contains(&"vip"),
        "the control: vip is reproduced when its rows stay"
    );
    let config = SynthesizeConfig {
        exclude_rows: rows_with_vip,
        ..base
    };
    let r = synthesize(&sample, &config).unwrap();
    assert!(
        !column(&r, "plan").contains(&"vip"),
        "removed rows must not influence the output"
    );
    assert_eq!((r.report.rows_in, r.report.rows_excluded), (498, 2));
}

#[test]
fn the_default_output_size_follows_the_rows_that_remain() {
    let excluded: std::collections::HashSet<usize> = (0..50).collect();
    let r = synthesize(
        &table(300),
        &SynthesizeConfig {
            exclude_rows: excluded,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.sample.rows.len(), 250);
}

#[test]
fn out_of_range_excluded_rows_remove_nothing() {
    let excluded: std::collections::HashSet<usize> = [10_000usize, 99_999].into_iter().collect();
    let r = synthesize(
        &table(300),
        &SynthesizeConfig {
            exclude_rows: excluded,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((r.report.rows_in, r.report.rows_excluded), (300, 0));
}
