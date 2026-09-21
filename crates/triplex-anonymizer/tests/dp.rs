//! Tier 2 slice T2.2: differential-privacy evaluation. Tests cannot prove DP,
//! so this checks what they can: refusal without a budget or bounds, an accountant that never
//! overspends, declared-domain output, no real-data figures in the default report, utility
//! that improves with epsilon, and an empirical membership-inference distinguisher compared with
//! the theoretical bound as a sanity check (not a proof).

use triplex_anonymizer::synthesize::dp::{
    config_from_declared, nonprivate_diagnostics, render_html, synthesize_dp, Domain, DpColumn,
    DpConfig, DpResult, InsecureSeededNoise, OsNoise,
};
use triplex_io::Sample;
use triplex_schemacast::parse_contract;

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

/// age ~ N(45,12); income correlated with age; plan depends on income; region independent; 10% of
/// income is null.
fn table(n: usize) -> Sample {
    let mut g = Gen(77);
    let rows = (0..n)
        .map(|_| {
            let age = (45.0 + 12.0 * g.normal()).clamp(18.0, 90.0).round();
            let income = (20_000.0 + 900.0 * age + 9_000.0 * g.normal()).clamp(0.0, 200_000.0);
            let plan = if income > 65_000.0 {
                "pro"
            } else if income > 50_000.0 {
                "plus"
            } else {
                "free"
            };
            let region = ["north", "south", "east", "west"][(g.next() % 4) as usize];
            vec![
                format!("{age}"),
                if g.uniform() < 0.1 {
                    String::new()
                } else {
                    format!("{income:.2}")
                },
                plan.to_string(),
                region.to_string(),
            ]
        })
        .collect();
    Sample {
        headers: ["age", "income", "plan", "region"]
            .iter()
            .map(|h| h.to_string())
            .collect(),
        rows,
    }
}

fn columns() -> Vec<DpColumn> {
    vec![
        DpColumn {
            name: "age".into(),
            domain: Domain::Numeric {
                min: 18.0,
                max: 90.0,
                integer: true,
                bins: 32,
            },
        },
        DpColumn {
            name: "income".into(),
            domain: Domain::Numeric {
                min: 0.0,
                max: 200_000.0,
                integer: false,
                bins: 32,
            },
        },
        DpColumn {
            name: "plan".into(),
            domain: Domain::Categorical {
                values: vec!["free".into(), "plus".into(), "pro".into()],
            },
        },
        DpColumn {
            name: "region".into(),
            domain: Domain::Categorical {
                values: vec!["north".into(), "south".into(), "east".into(), "west".into()],
            },
        },
    ]
}

fn config(epsilon: f64) -> DpConfig {
    DpConfig {
        epsilon,
        marginal_share: 0.5,
        columns: columns(),
        rows: None,
        sampling_seed: 1,
        exclude_rows: Default::default(),
    }
}

fn run(n: usize, epsilon: f64, noise_seed: u64) -> DpResult {
    synthesize_dp(
        &table(n),
        &config(epsilon),
        &mut InsecureSeededNoise::new(noise_seed),
    )
    .unwrap()
}

fn column<'a>(r: &'a DpResult, name: &str) -> Vec<&'a str> {
    let i = r.sample.headers.iter().position(|h| h == name).unwrap();
    r.sample.rows.iter().map(|row| row[i].as_str()).collect()
}

// ---- refusal ----------------------------------------------------------------------------

fn declared(yaml: &str) -> triplex_schemacast::DeclaredSynthesize {
    parse_contract(&format!(
        "version: 1\ncolumns: []\nsynthesize:\n{}",
        yaml.lines().map(|l| format!("  {l}\n")).collect::<String>()
    ))
    .unwrap()
    .synthesize
    .unwrap()
}

#[test]
fn a_missing_epsilon_is_refused_because_there_is_no_default() {
    let d = declared("columns:\n  - name: age\n    type: numeric\n    min: 0\n    max: 100");
    let e = config_from_declared(&d, None, 1).unwrap_err().to_string();
    assert!(e.contains("no default"), "{e}");
}

#[test]
fn numeric_columns_without_declared_bounds_are_refused() {
    let d = declared("epsilon: 1.0\ncolumns:\n  - name: age\n    type: numeric\n    min: 0");
    let e = config_from_declared(&d, None, 1).unwrap_err().to_string();
    assert!(e.contains("public bounds"), "{e}");
}

#[test]
fn a_complete_block_becomes_a_config_with_the_documented_defaults() {
    let d = declared("epsilon: 2.5\ncolumns:\n  - name: age\n    type: numeric\n    min: 18\n    max: 90\n    integer: true\n  - name: plan\n    type: categorical\n    values: [free, pro]");
    let c = config_from_declared(&d, Some(50), 9).unwrap();
    assert_eq!(
        (c.epsilon, c.marginal_share, c.rows, c.sampling_seed),
        (2.5, 0.5, Some(50), 9)
    );
    assert_eq!(c.columns.len(), 2);
    assert!(matches!(
        c.columns[0].domain,
        Domain::Numeric {
            integer: true,
            bins: 32,
            ..
        }
    ));
}

#[test]
fn invalid_budgets_shares_and_columns_are_refused() {
    let real = table(100);
    let bad = |mutate: &dyn Fn(&mut DpConfig)| {
        let mut c = config(1.0);
        mutate(&mut c);
        synthesize_dp(&real, &c, &mut InsecureSeededNoise::new(1)).is_err()
    };
    assert!(bad(&|c| c.epsilon = 0.0));
    assert!(bad(&|c| c.epsilon = -1.0));
    assert!(bad(&|c| c.epsilon = f64::NAN));
    assert!(bad(&|c| c.epsilon = f64::INFINITY));
    assert!(bad(&|c| c.marginal_share = 0.0));
    assert!(bad(&|c| c.marginal_share = 1.0));
    assert!(bad(&|c| c.columns.clear()));
    assert!(bad(&|c| c.columns[0].name = "missing".into()));
    let dup = columns()[0].clone();
    assert!(bad(&|c| c.columns.push(dup.clone())));
}

#[test]
fn many_rows_with_two_numeric_columns_are_refused_but_one_numeric_column_is_fine() {
    let big = Sample {
        headers: vec!["a".into(), "b".into()],
        rows: (0..30_001)
            .map(|i| vec![(i % 90).to_string(), (i % 70).to_string()])
            .collect(),
    };
    let two_numeric: Vec<DpColumn> = ["a", "b"]
        .iter()
        .map(|n| DpColumn {
            name: n.to_string(),
            domain: Domain::Numeric {
                min: 0.0,
                max: 100.0,
                integer: true,
                bins: 16,
            },
        })
        .collect();
    let cfg = |cols: Vec<DpColumn>| DpConfig {
        epsilon: 1.0,
        marginal_share: 0.5,
        columns: cols,
        rows: Some(10),
        sampling_seed: 1,
        exclude_rows: Default::default(),
    };
    assert!(synthesize_dp(
        &big,
        &cfg(two_numeric.clone()),
        &mut InsecureSeededNoise::new(1)
    )
    .is_err());
    assert!(synthesize_dp(
        &big,
        &cfg(vec![two_numeric[0].clone()]),
        &mut InsecureSeededNoise::new(1)
    )
    .is_ok());
}

// ---- the accountant and the split -------------------------------------------------------

#[test]
fn the_ledger_never_spends_more_than_the_budget_and_uses_rounded_down_epsilons() {
    for eps in [0.1, 1.0, 3.7, 25.0] {
        let r = run(400, eps, 1);
        let ledger = &r.report.privacy.ledger;
        let spent: f64 = ledger.iter().map(|s| s.epsilon_used).sum();
        assert!(spent <= eps * (1.0 + 1e-9), "eps {eps}: spent {spent}");
        assert!((r.report.privacy.epsilon_spent - spent).abs() < 1e-9);
        assert!(ledger
            .iter()
            .all(|s| s.epsilon_used <= s.epsilon_allotted && s.epsilon_used > 0.0));
        assert_eq!(r.report.privacy.epsilon_total, eps);
    }
}

#[test]
fn the_budget_is_split_between_marginals_and_correlations_as_configured() {
    let r = run(400, 2.0, 1);
    let ledger = &r.report.privacy.ledger;
    assert_eq!(ledger.len(), 4 + 1, "four histograms and one numeric pair");
    let marginals: f64 = ledger
        .iter()
        .filter(|s| s.mechanism.starts_with("histogram:"))
        .map(|s| s.epsilon_used)
        .sum();
    let correlation: f64 = ledger
        .iter()
        .filter(|s| s.mechanism.starts_with("kendall:"))
        .map(|s| s.epsilon_used)
        .sum();
    assert!(
        (marginals - 1.0).abs() < 1e-4 && (correlation - 1.0).abs() < 1e-4,
        "{marginals} / {correlation}"
    );
    let k = ledger
        .iter()
        .find(|s| s.mechanism.starts_with("kendall:"))
        .unwrap();
    assert_eq!(
        k.sensitivity,
        2 * (400 - 1),
        "Kendall sensitivity is 2(n - 1)"
    );
    assert!(ledger
        .iter()
        .filter(|s| s.mechanism.starts_with("histogram:"))
        .all(|s| s.sensitivity == 2));
}

#[test]
fn with_no_numeric_pair_the_marginals_receive_the_whole_budget() {
    let real = table(300);
    let cfg = DpConfig {
        epsilon: 1.0,
        marginal_share: 0.5,
        columns: columns()[2..].to_vec(),
        rows: None,
        sampling_seed: 1,
        exclude_rows: Default::default(),
    };
    let r = synthesize_dp(&real, &cfg, &mut InsecureSeededNoise::new(1)).unwrap();
    assert!(r
        .report
        .privacy
        .ledger
        .iter()
        .all(|s| s.mechanism.starts_with("histogram:")));
    assert!((r.report.privacy.epsilon_spent - 1.0).abs() < 1e-4);
}

// ---- output ------------------------------------------------------------------------------

#[test]
fn every_synthetic_value_lies_inside_the_declared_public_domain() {
    let r = run(500, 1.0, 3);
    assert!(column(&r, "age").iter().filter(|v| !v.is_empty()).all(|v| {
        let x: i64 = v.parse().unwrap();
        (18..=90).contains(&x)
    }));
    assert!(column(&r, "income")
        .iter()
        .filter(|v| !v.is_empty())
        .all(|v| {
            let x: f64 = v.parse().unwrap();
            (0.0..=200_000.0).contains(&x)
        }));
    assert!(column(&r, "plan")
        .iter()
        .all(|v| ["free", "plus", "pro", "(other)", ""].contains(v)));
    assert!(column(&r, "region")
        .iter()
        .all(|v| ["north", "south", "east", "west", "(other)", ""].contains(v)));
    assert_eq!(r.sample.headers, ["age", "income", "plan", "region"]);
}

#[test]
fn the_row_count_defaults_to_the_input_and_can_be_overridden() {
    assert_eq!(run(300, 1.0, 1).sample.rows.len(), 300);
    let mut cfg = config(1.0);
    cfg.rows = Some(77);
    let r = synthesize_dp(&table(300), &cfg, &mut InsecureSeededNoise::new(1)).unwrap();
    assert_eq!(
        (r.sample.rows.len(), r.report.rows_in, r.report.rows_out),
        (77, 300, 77)
    );
}

#[test]
fn sampling_is_seeded_but_noise_is_independent_of_the_sampling_seed() {
    let a = run(300, 1.0, 5);
    let b = run(300, 1.0, 5);
    assert_eq!(
        a.sample.rows, b.sample.rows,
        "same noise and sampling seeds"
    );
    let c = run(300, 1.0, 6);
    assert_ne!(a.sample.rows, c.sample.rows, "different noise");
    let mut cfg = config(1.0);
    cfg.sampling_seed = 99;
    let d = synthesize_dp(&table(300), &cfg, &mut InsecureSeededNoise::new(5)).unwrap();
    assert_ne!(a.sample.rows, d.sample.rows, "different sampling seed");
    assert_eq!(
        a.report.privacy.ledger, d.report.privacy.ledger,
        "the budget does not depend on the sampling seed"
    );
}

#[test]
fn operating_system_noise_gives_a_different_release_every_run() {
    let cfg = config(1.0);
    let real = table(300);
    let a = synthesize_dp(&real, &cfg, &mut OsNoise).unwrap();
    let b = synthesize_dp(&real, &cfg, &mut OsNoise).unwrap();
    assert_ne!(
        a.sample.rows, b.sample.rows,
        "same sampling seed, so any difference is the noise"
    );
}

// ---- what the report may and may not contain -----------------------------------------------

#[test]
fn the_default_report_has_no_real_data_figures_and_states_the_guarantee() {
    let r = run(300, 1.5, 1);
    let json = serde_json::to_string(&r.report).unwrap();
    for key in [
        "fidelity",
        "leakage",
        "nonprivate_diagnostics",
        "exact_copy",
        "ks_statistic",
    ] {
        assert!(
            !json.contains(key),
            "the default report must not contain {key:?}"
        );
    }
    let p = &r.report.privacy;
    assert_eq!(
        (
            p.epsilon_total,
            p.delta,
            p.neighbor_relation,
            p.row_count_public
        ),
        (1.5, 0.0, "replace one record", true)
    );
    assert!(
        p.statement.contains("epsilon-differentially private")
            && p.statement.contains("post-processing")
    );
    assert!(p.noise_source.contains("operating system"));
}

#[test]
fn diagnostics_are_attached_only_on_request_under_a_warning() {
    let real = table(400);
    let cfg = config(5.0);
    let r = synthesize_dp(&real, &cfg, &mut InsecureSeededNoise::new(1)).unwrap();
    let diagnostics = nonprivate_diagnostics(&real, &r.sample, &cfg).unwrap();
    assert!(diagnostics
        .warning
        .contains("NOT covered by the privacy guarantee"));
    let with = r.report.with_diagnostics(diagnostics);
    let json = serde_json::to_string(&with).unwrap();
    assert!(
        json.contains("nonprivate_diagnostics")
            && json.contains("fidelity")
            && json.contains("leakage")
    );
    assert!(render_html(&with).contains("Non-private diagnostics"));
}

#[test]
fn no_output_claims_anonymity_or_safety_and_the_html_is_self_contained() {
    let r = run(200, 1.0, 1);
    let html = render_html(&r.report);
    let json = serde_json::to_string(&r.report).unwrap();
    for (kind, text) in [("html", html.to_lowercase()), ("json", json.to_lowercase())] {
        for claim in [
            "anonymous",
            "safe to share",
            "unbreakable",
            "no risk",
            "guarantees privacy",
        ] {
            assert!(!text.contains(claim), "{kind} contains {claim:?}");
        }
    }
    assert!(
        html.contains("epsilon = 1") && !html.contains("http://") && !html.contains("https://")
    );
    assert!(
        !html.contains("Non-private diagnostics"),
        "absent unless attached"
    );
}

// ---- utility -----------------------------------------------------------------------------

fn fidelity_error(epsilon: f64, noise_seed: u64) -> (f64, f64) {
    let real = table(3000);
    let cfg = config(epsilon);
    let r = synthesize_dp(&real, &cfg, &mut InsecureSeededNoise::new(noise_seed)).unwrap();
    let d = nonprivate_diagnostics(&real, &r.sample, &cfg).unwrap();
    let ks: f64 = d
        .fidelity
        .columns
        .iter()
        .filter_map(|c| c.ks_statistic)
        .sum::<f64>()
        / 2.0;
    (ks, d.fidelity.correlation.max_abs_error.unwrap_or(0.0))
}

#[test]
fn at_a_very_large_epsilon_the_output_is_close_to_the_non_private_copula() {
    let (ks, corr) = fidelity_error(2000.0, 1);
    assert!(ks < 0.06, "mean KS {ks}");
    assert!(corr < 0.12, "correlation error {corr}");
}

#[test]
fn utility_improves_as_epsilon_grows() {
    let mean = |eps: f64| (1..=5).map(|s| fidelity_error(eps, s).0).sum::<f64>() / 5.0;
    let (loose, tight) = (mean(0.05), mean(50.0));
    assert!(
        tight < loose,
        "mean KS at epsilon 50 ({tight}) should beat epsilon 0.05 ({loose})"
    );
}

#[test]
fn a_tiny_epsilon_still_produces_a_valid_release_within_the_domain() {
    let r = run(300, 0.02, 1);
    assert_eq!(r.sample.rows.len(), 300);
    assert!(column(&r, "age")
        .iter()
        .filter(|v| !v.is_empty())
        .all(|v| (18..=90).contains(&v.parse::<i64>().unwrap())));
}

#[test]
fn a_null_rate_survives_at_a_generous_epsilon() {
    let r = run(3000, 500.0, 1);
    let nulls = column(&r, "income").iter().filter(|v| v.is_empty()).count() as f64 / 3000.0;
    assert!((nulls - 0.1).abs() < 0.03, "null rate {nulls}");
}

// ---- an empirical distinguisher, as a sanity check (not a proof) ---------------------------

/// Accuracy of the best threshold test at telling two neighboring datasets apart from the share of
/// "a" in the synthetic output. DP bounds it by e^eps / (1 + e^eps).
fn distinguisher_accuracy(epsilon: f64, trials: u64) -> f64 {
    let dataset = |a: usize, b: usize| Sample {
        headers: vec!["plan".into()],
        rows: std::iter::repeat_n(vec!["a".to_string()], a)
            .chain(std::iter::repeat_n(vec!["b".to_string()], b))
            .collect(),
    };
    // neighbors: one record of the first dataset is replaced ("a" becomes "b")
    let (d0, d1) = (dataset(100, 100), dataset(99, 101));
    let cfg = DpConfig {
        epsilon,
        marginal_share: 0.5,
        columns: vec![DpColumn {
            name: "plan".into(),
            domain: Domain::Categorical {
                values: vec!["a".into(), "b".into()],
            },
        }],
        rows: Some(20_000),
        sampling_seed: 1,
        exclude_rows: Default::default(),
    };
    let share = |d: &Sample, seed: u64| {
        let r = synthesize_dp(d, &cfg, &mut InsecureSeededNoise::new(seed)).unwrap();
        r.sample.rows.iter().filter(|row| row[0] == "a").count() as f64 / r.sample.rows.len() as f64
    };
    let s0: Vec<f64> = (0..trials).map(|t| share(&d0, 1000 + t)).collect();
    let s1: Vec<f64> = (0..trials).map(|t| share(&d1, 500_000 + t)).collect();
    let (m0, m1) = (
        s0.iter().sum::<f64>() / trials as f64,
        s1.iter().sum::<f64>() / trials as f64,
    );
    let threshold = (m0 + m1) / 2.0;
    let higher_is_d0 = m0 > m1;
    let correct = s0
        .iter()
        .filter(|&&v| (v > threshold) == higher_is_d0)
        .count()
        + s1.iter()
            .filter(|&&v| (v > threshold) != higher_is_d0)
            .count();
    correct as f64 / (2 * trials) as f64
}

#[test]
fn an_empirical_distinguisher_stays_within_the_theoretical_dp_bound() {
    let epsilon: f64 = 1.0;
    let bound = epsilon.exp() / (1.0 + epsilon.exp());
    let accuracy = distinguisher_accuracy(epsilon, 400);
    assert!(
        accuracy <= bound + 0.05,
        "accuracy {accuracy} exceeds the epsilon-{epsilon} bound {bound} (plus slack)"
    );
}

#[test]
fn the_distinguisher_gets_better_as_epsilon_grows_so_the_test_can_detect_leakage() {
    let (low, high) = (
        distinguisher_accuracy(0.2, 300),
        distinguisher_accuracy(60.0, 300),
    );
    assert!(
        high > low + 0.05,
        "accuracy at epsilon 60 ({high}) should clearly exceed epsilon 0.2 ({low})"
    );
}

// ---- gaps found by mutation testing (cargo-mutants, 2026-09-20) ------------------------------

fn error_of(mutate: &dyn Fn(&mut DpConfig)) -> String {
    let mut c = config(1.0);
    mutate(&mut c);
    synthesize_dp(&table(100), &c, &mut InsecureSeededNoise::new(1))
        .err()
        .unwrap()
        .to_string()
}

#[test]
fn a_bad_epsilon_and_a_bad_share_are_each_refused_with_their_own_message() {
    for eps in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let e = error_of(&|c| c.epsilon = eps);
        assert!(e.contains("positive privacy budget"), "epsilon {eps}: {e}");
    }
    for share in [0.0, 1.0, -0.1, 1.5, f64::NAN] {
        let e = error_of(&|c| c.marginal_share = share);
        assert!(e.contains("strictly between 0 and 1"), "share {share}: {e}");
    }
    let ok = |share: f64| {
        let mut c = config(1.0);
        c.marginal_share = share;
        synthesize_dp(&table(100), &c, &mut InsecureSeededNoise::new(1)).is_ok()
    };
    assert!(
        ok(0.001) && ok(0.999),
        "values just inside (0, 1) are valid"
    );
}

fn three_numeric() -> Vec<DpColumn> {
    ["a", "b", "c"]
        .iter()
        .map(|n| DpColumn {
            name: n.to_string(),
            domain: Domain::Numeric {
                min: 0.0,
                max: 100.0,
                integer: true,
                bins: 16,
            },
        })
        .collect()
}

#[test]
fn several_pairs_share_the_correlation_budget_equally() {
    let real = Sample {
        headers: vec!["a".into(), "b".into(), "c".into()],
        rows: (0..200)
            .map(|i| {
                vec![
                    (i % 90).to_string(),
                    ((i * 7) % 80).to_string(),
                    ((i * 3) % 70).to_string(),
                ]
            })
            .collect(),
    };
    let cfg = DpConfig {
        epsilon: 3.0,
        marginal_share: 0.4,
        columns: three_numeric(),
        rows: Some(10),
        sampling_seed: 1,
        exclude_rows: Default::default(),
    };
    let r = synthesize_dp(&real, &cfg, &mut InsecureSeededNoise::new(1)).unwrap();
    let kendall: Vec<f64> = r
        .report
        .privacy
        .ledger
        .iter()
        .filter(|s| s.mechanism.starts_with("kendall:"))
        .map(|s| s.epsilon_used)
        .collect();
    let histograms: Vec<f64> = r
        .report
        .privacy
        .ledger
        .iter()
        .filter(|s| s.mechanism.starts_with("histogram:"))
        .map(|s| s.epsilon_used)
        .collect();
    assert_eq!((kendall.len(), histograms.len()), (3, 3));
    // marginals get 0.4 * 3 = 1.2 (0.4 each); correlations get 1.8 (0.6 per pair)
    assert!(
        histograms.iter().all(|e| (e - 0.4).abs() < 1e-5),
        "{histograms:?}"
    );
    assert!(
        kendall.iter().all(|e| (e - 0.6).abs() < 1e-5),
        "{kendall:?}"
    );
}

#[test]
fn a_normal_release_reports_categorical_independence_and_no_repair() {
    let healthy = run(300, 1.0, 1);
    assert!(healthy
        .report
        .warnings
        .iter()
        .any(|w| w.contains("categorical columns are sampled independently")));
    assert_eq!(healthy.report.correlation_shrinkage, 0.0);
    assert!(!healthy.report.warnings.iter().any(|w| w.contains("shrunk")));
}

#[test]
fn diagnostics_map_undeclared_real_categories_to_other_like_the_mechanism_does() {
    // 20% of the real rows carry a value the config did not declare; at a huge epsilon the
    // synthetic data reproduces that share as "(other)", so the total variation must be small.
    let rows: Vec<Vec<String>> = (0..2000)
        .map(|i| {
            vec![if i % 5 == 0 {
                "zzz"
            } else if i % 2 == 0 {
                "a"
            } else {
                "b"
            }
            .to_string()]
        })
        .collect();
    let real = Sample {
        headers: vec!["k".into()],
        rows,
    };
    let cfg = DpConfig {
        epsilon: 5000.0,
        marginal_share: 0.5,
        columns: vec![DpColumn {
            name: "k".into(),
            domain: Domain::Categorical {
                values: vec!["a".into(), "b".into()],
            },
        }],
        rows: None,
        sampling_seed: 1,
        exclude_rows: Default::default(),
    };
    let r = synthesize_dp(&real, &cfg, &mut InsecureSeededNoise::new(1)).unwrap();
    assert!(column(&r, "k").contains(&"(other)"));
    let d = nonprivate_diagnostics(&real, &r.sample, &cfg).unwrap();
    let tv = d.fidelity.columns[0].total_variation.unwrap();
    assert!(tv < 0.05, "total variation {tv}");
}

// ---- rows the user removed ---------------------------------------------------------------------

#[test]
fn excluded_rows_shrink_the_public_row_count_and_the_kendall_sensitivity() {
    let mut cfg = config(2.0);
    cfg.exclude_rows = (0..100).collect();
    let r = synthesize_dp(&table(400), &cfg, &mut InsecureSeededNoise::new(1)).unwrap();
    assert_eq!(
        (
            r.report.rows_in,
            r.report.rows_excluded,
            r.sample.rows.len()
        ),
        (300, 100, 300)
    );
    let k = r
        .report
        .privacy
        .ledger
        .iter()
        .find(|s| s.mechanism.starts_with("kendall:"))
        .unwrap();
    assert_eq!(
        k.sensitivity,
        2 * (300 - 1),
        "sensitivity is computed for the rows that remain"
    );
}

#[test]
fn out_of_range_excluded_rows_change_nothing_in_a_dp_release() {
    let mut cfg = config(1.0);
    cfg.exclude_rows = [50_000usize].into_iter().collect();
    let with = synthesize_dp(&table(200), &cfg, &mut InsecureSeededNoise::new(4)).unwrap();
    let without =
        synthesize_dp(&table(200), &config(1.0), &mut InsecureSeededNoise::new(4)).unwrap();
    assert_eq!(with.sample.rows, without.sample.rows);
    assert_eq!(with.report.rows_excluded, 0);
}
