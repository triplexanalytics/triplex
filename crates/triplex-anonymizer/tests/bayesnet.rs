//! Tier 2, slice T2.3: the Bayesian-network synthesizers. Synthetic fixtures
//! only. The point of the method is to keep dependencies the Gaussian copula cannot: between
//! categorical columns (Chow-Liu tree) and three-way effects (k-parent network).

use triplex_anonymizer::synthesize::bayesnet::dp::{synthesize_bayes_dp, BayesDpConfig};
use triplex_anonymizer::synthesize::bayesnet::{synthesize_bayes, BayesConfig, StructureKind};
use triplex_anonymizer::synthesize::dp::{
    synthesize_dp, Domain, DpColumn, DpConfig, InsecureSeededNoise,
};
use triplex_anonymizer::synthesize::SynthesizeConfig;
use triplex_io::Sample;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f64 {
        (self.next() % 1_000_000) as f64 / 1_000_000.0
    }
}

const PLANS: [&str; 3] = ["free", "pro", "enterprise"];
const REGIONS: [&str; 4] = ["north", "south", "east", "west"];

/// `region` follows `plan` 90% of the time (free -> north, pro -> south, enterprise -> east);
/// `segment` and `age` are independent noise.
fn plan_region(n: usize) -> Sample {
    let mut rng = Lcg(7);
    let rows = (0..n)
        .map(|_| {
            let p = match rng.unit() {
                x if x < 0.5 => 0,
                x if x < 0.8 => 1,
                _ => 2,
            };
            let region = if rng.unit() < 0.9 {
                p
            } else {
                rng.below(4) as usize
            };
            vec![
                PLANS[p].to_string(),
                REGIONS[region].to_string(),
                ["a", "b", "c", "d"][rng.below(4) as usize].to_string(),
                (18 + rng.below(70)).to_string(),
            ]
        })
        .collect();
    Sample {
        headers: vec![
            "plan".into(),
            "region".into(),
            "segment".into(),
            "age".into(),
        ],
        rows,
    }
}

/// `out = left XOR right`, with `left` and `right` independent fair coins: no pair of columns is
/// correlated, only the three together.
fn xor(n: usize) -> Sample {
    let mut rng = Lcg(11);
    let rows = (0..n)
        .map(|_| {
            let (l, r) = (rng.below(2), rng.below(2));
            vec![l.to_string(), r.to_string(), (l ^ r).to_string()]
        })
        .collect();
    Sample {
        headers: vec!["left".into(), "right".into(), "out".into()],
        rows,
    }
}

fn agreement(sample: &Sample) -> f64 {
    let (plan, region) = (0, 1);
    let hits = sample
        .rows
        .iter()
        .filter(|r| {
            let p = PLANS.iter().position(|x| *x == r[plan]);
            let g = REGIONS.iter().position(|x| *x == r[region]);
            matches!((p, g), (Some(p), Some(g)) if p == g)
        })
        .count();
    hits as f64 / sample.rows.len() as f64
}

fn xor_holds(sample: &Sample) -> f64 {
    let hits = sample
        .rows
        .iter()
        .filter(|r| {
            let v: Vec<u8> = r.iter().map(|x| x.parse().unwrap()).collect();
            v[0] ^ v[1] == v[2]
        })
        .count();
    hits as f64 / sample.rows.len() as f64
}

fn base(seed: u64) -> SynthesizeConfig {
    SynthesizeConfig {
        seed,
        ..SynthesizeConfig::default()
    }
}

fn dp_columns(with_age: bool) -> Vec<DpColumn> {
    let cat = |name: &str, values: &[&str]| DpColumn {
        name: name.into(),
        domain: Domain::Categorical {
            values: values.iter().map(|v| v.to_string()).collect(),
        },
    };
    let mut columns = vec![
        cat("plan", &PLANS),
        cat("region", &REGIONS),
        cat("segment", &["a", "b", "c", "d"]),
    ];
    if with_age {
        columns.push(DpColumn {
            name: "age".into(),
            domain: Domain::Numeric {
                min: 18.0,
                max: 88.0,
                integer: true,
                bins: 8,
            },
        });
    }
    columns
}

fn dp_config(epsilon: f64, columns: Vec<DpColumn>, seed: u64) -> DpConfig {
    DpConfig {
        epsilon,
        marginal_share: 0.5,
        columns,
        rows: None,
        sampling_seed: seed,
        exclude_rows: Default::default(),
    }
}

// ---- without differential privacy

#[test]
fn the_tree_keeps_a_categorical_dependency_the_copula_loses() {
    let real = plan_region(4000);
    let result = synthesize_bayes(
        &real,
        &BayesConfig {
            base: base(1),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.sample.rows.len(), 4000);
    let network = result.report.network.as_ref().expect("a network report");
    assert_eq!(network.structure, "chow_liu");
    let region = network.nodes.iter().find(|n| n.column == "region").unwrap();
    assert_eq!(region.parents, vec!["plan".to_string()]);
    let (real_agreement, synth_agreement) = (agreement(&real), agreement(&result.sample));
    assert!(
        (real_agreement - synth_agreement).abs() < 0.04,
        "real {real_agreement}, synthetic {synth_agreement}"
    );

    // The copula samples categorical columns independently of the others (DP variant), so the same
    // dependency collapses to chance.
    let mut noise = InsecureSeededNoise::new(9);
    let copula =
        synthesize_dp(&real, &dp_config(1000.0, dp_columns(false), 1), &mut noise).unwrap();
    assert!(
        agreement(&copula.sample) < real_agreement - 0.2,
        "copula {}",
        agreement(&copula.sample)
    );
}

#[test]
fn only_a_two_parent_network_keeps_a_three_way_effect() {
    let real = xor(4000);
    let tree = synthesize_bayes(
        &real,
        &BayesConfig {
            base: base(2),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        xor_holds(&tree.sample) < 0.6,
        "a tree cannot see an XOR: {}",
        xor_holds(&tree.sample)
    );
    let k2 = synthesize_bayes(
        &real,
        &BayesConfig {
            base: base(2),
            structure: StructureKind::KParent,
            max_parents: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        xor_holds(&k2.sample) > 0.95,
        "k=2: {}",
        xor_holds(&k2.sample)
    );
    let out = k2
        .report
        .network
        .unwrap()
        .nodes
        .into_iter()
        .find(|n| n.column == "out")
        .unwrap();
    assert_eq!(out.parents.len(), 2);
}

#[test]
fn numbers_stay_inside_their_range_and_marginals_match() {
    let real = plan_region(4000);
    let result = synthesize_bayes(
        &real,
        &BayesConfig {
            base: base(3),
            ..Default::default()
        },
    )
    .unwrap();
    let ages: Vec<i64> = result
        .sample
        .rows
        .iter()
        .map(|r| r[3].parse().unwrap())
        .collect();
    assert!(
        ages.iter().all(|a| (18..=87).contains(a)),
        "ages out of range"
    );
    let free =
        |s: &Sample| s.rows.iter().filter(|r| r[0] == "free").count() as f64 / s.rows.len() as f64;
    assert!((free(&real) - free(&result.sample)).abs() < 0.03);
    assert!(result
        .report
        .fidelity
        .columns
        .iter()
        .all(|c| c.total_variation.unwrap_or(0.0) < 0.06 && c.ks_statistic.unwrap_or(0.0) < 0.08));
}

#[test]
fn the_same_seed_gives_the_same_table_and_a_different_seed_a_different_one() {
    let real = plan_region(500);
    let run = |seed| {
        synthesize_bayes(
            &real,
            &BayesConfig {
                base: base(seed),
                ..Default::default()
            },
        )
        .unwrap()
        .sample
        .rows
    };
    assert_eq!(run(5), run(5));
    assert_ne!(run(5), run(6));
}

#[test]
fn a_bad_shape_is_a_clear_error_and_removed_rows_and_columns_are_honored() {
    let real = plan_region(300);
    for k in [0, 4] {
        let err = synthesize_bayes(
            &real,
            &BayesConfig {
                structure: StructureKind::KParent,
                max_parents: k,
                ..Default::default()
            },
        )
        .err()
        .unwrap();
        assert!(err.to_string().contains("max_parents"), "{err}");
    }
    let mut base = base(1);
    base.exclude_rows = (0..100).collect();
    base.exclude_columns = vec!["segment".into()];
    let result = synthesize_bayes(
        &real,
        &BayesConfig {
            base,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.report.rows_excluded, 100);
    assert_eq!(result.report.rows_in, 200);
    assert!(!result.sample.headers.contains(&"segment".to_string()));
}

// ---- with differential privacy

fn bn_dp(
    epsilon: f64,
    kind: StructureKind,
    k: usize,
    seed: u64,
    columns: Vec<DpColumn>,
) -> BayesDpConfig {
    BayesDpConfig {
        dp: dp_config(epsilon, columns, seed),
        structure: kind,
        max_parents: k,
        structure_share: 0.3,
    }
}

#[test]
fn a_generous_budget_keeps_the_dependency_and_the_budget_is_fully_accounted() {
    let real = plan_region(6000);
    let mut noise = InsecureSeededNoise::new(21);
    let config = bn_dp(50.0, StructureKind::ChowLiu, 1, 1, dp_columns(true));
    let result = synthesize_bayes_dp(&real, &config, &mut noise).unwrap();
    assert_eq!(result.sample.rows.len(), 6000);
    let network = result.report.network.as_ref().unwrap();
    let region = network.nodes.iter().find(|n| n.column == "region").unwrap();
    assert_eq!(region.parents, vec!["plan".to_string()]);
    assert!(
        (agreement(&result.sample) - agreement(&real)).abs() < 0.05,
        "{}",
        agreement(&result.sample)
    );

    let p = &result.report.privacy;
    assert_eq!(p.epsilon_total, 50.0);
    assert!(
        (p.epsilon_spent - 50.0).abs() < 1e-6,
        "all of the budget is accounted for: {}",
        p.epsilon_spent
    );
    // one report-noisy-max choice per non-root column, one table per column
    let choices = p
        .ledger
        .iter()
        .filter(|s| s.mechanism.starts_with("structure:"))
        .count();
    let tables = p
        .ledger
        .iter()
        .filter(|s| s.mechanism.starts_with("table:"))
        .count();
    assert_eq!((choices, tables), (3, 4));
    let structure_spent: f64 = p
        .ledger
        .iter()
        .filter(|s| s.mechanism.starts_with("structure:"))
        .map(|s| s.epsilon_used)
        .sum();
    assert!(
        (structure_spent - 0.3 * 50.0).abs() < 1e-3,
        "{structure_spent}"
    );
    assert!(p
        .ledger
        .iter()
        .all(|s| s.epsilon_used <= s.epsilon_allotted));
    assert!(result.report.nonprivate_diagnostics.is_none());
    assert_eq!(result.report.method, "bayesian_network_chow_liu_dp");
}

#[test]
fn a_small_budget_still_runs_stays_within_budget_and_warns_that_tables_are_noise() {
    let real = plan_region(500);
    let mut noise = InsecureSeededNoise::new(22);
    let config = bn_dp(0.05, StructureKind::KParent, 2, 1, dp_columns(true));
    let result = synthesize_bayes_dp(&real, &config, &mut noise).unwrap();
    assert_eq!(result.sample.rows.len(), 500);
    assert!(result.report.privacy.epsilon_spent <= 0.05 * (1.0 + 1e-6));
    assert!(
        result
            .report
            .warnings
            .iter()
            .any(|w| w.contains("mostly noise")),
        "{:?}",
        result.report.warnings
    );
    // every value is inside its declared domain
    for row in &result.sample.rows {
        assert!(PLANS.contains(&row[0].as_str()) || row[0] == "(other)" || row[0].is_empty());
        let age: i64 = row[3].parse().unwrap_or(18);
        assert!((18..=88).contains(&age));
    }
}

#[test]
fn noise_does_not_invent_nulls_or_other_values_in_columns_that_have_none() {
    let real = plan_region(3000);
    for (kind, k) in [(StructureKind::ChowLiu, 1), (StructureKind::KParent, 2)] {
        let mut noise = InsecureSeededNoise::new(24);
        let config = bn_dp(1.0, kind, k, 1, dp_columns(true));
        let result = synthesize_bayes_dp(&real, &config, &mut noise).unwrap();
        for column in 0..4 {
            let odd = result
                .sample
                .rows
                .iter()
                .filter(|r| r[column].is_empty() || r[column] == "(other)")
                .count();
            assert!(
                (odd as f64 / 3000.0) < 0.02,
                "{kind:?} column {column}: {odd} empty or (other) cells the data never had"
            );
        }
    }
}

#[test]
fn k_parent_under_dp_keeps_a_three_way_effect_with_a_large_budget() {
    let real = xor(6000);
    let columns = ["left", "right", "out"]
        .iter()
        .map(|n| DpColumn {
            name: n.to_string(),
            domain: Domain::Categorical {
                values: vec!["0".into(), "1".into()],
            },
        })
        .collect();
    let mut noise = InsecureSeededNoise::new(23);
    let result = synthesize_bayes_dp(
        &real,
        &bn_dp(200.0, StructureKind::KParent, 2, 1, columns),
        &mut noise,
    )
    .unwrap();
    assert!(
        xor_holds(&result.sample) > 0.9,
        "{}",
        xor_holds(&result.sample)
    );
}

#[test]
fn the_dp_output_depends_only_on_the_declared_domains_the_noise_and_the_seed() {
    let real = plan_region(800);
    let run = |noise_seed, sampling| {
        let mut noise = InsecureSeededNoise::new(noise_seed);
        synthesize_bayes_dp(
            &real,
            &bn_dp(2.0, StructureKind::ChowLiu, 1, sampling, dp_columns(true)),
            &mut noise,
        )
        .unwrap()
        .sample
        .rows
    };
    assert_eq!(run(1, 1), run(1, 1));
    assert_ne!(run(1, 1), run(2, 1), "the noise changes the model");
    assert_ne!(run(1, 1), run(1, 2), "the sampling seed changes the draws");
}

#[test]
fn dp_validation_errors_are_clear() {
    let real = plan_region(100);
    let mut noise = InsecureSeededNoise::new(1);
    let ok = bn_dp(1.0, StructureKind::ChowLiu, 1, 1, dp_columns(true));
    let err = |c: BayesDpConfig| {
        synthesize_bayes_dp(&real, &c, &mut InsecureSeededNoise::new(1))
            .err()
            .unwrap()
            .to_string()
    };
    assert!(err(BayesDpConfig {
        dp: dp_config(0.0, dp_columns(true), 1),
        ..ok.clone()
    })
    .contains("epsilon"));
    assert!(err(BayesDpConfig {
        structure_share: 1.0,
        ..ok.clone()
    })
    .contains("structure_share"));
    assert!(err(BayesDpConfig {
        structure: StructureKind::KParent,
        max_parents: 9,
        ..ok.clone()
    })
    .contains("max_parents"));
    assert!(err(BayesDpConfig {
        dp: dp_config(1.0, vec![], 1),
        ..ok.clone()
    })
    .contains("at least one column"));
    let mut missing = dp_columns(true);
    missing.push(DpColumn {
        name: "nope".into(),
        domain: Domain::Categorical {
            values: vec!["x".into()],
        },
    });
    assert!(err(BayesDpConfig {
        dp: dp_config(1.0, missing, 1),
        ..ok.clone()
    })
    .contains("nope"));
    // removed rows shrink the public row count, and are reported
    let mut config = ok;
    config.dp.exclude_rows = (0..40).collect();
    let result = synthesize_bayes_dp(&real, &config, &mut noise).unwrap();
    assert_eq!(
        (result.report.rows_excluded, result.report.rows_in),
        (40, 60)
    );
}

// ---- slice T2.4: independent marginals, the baseline

/// Share of rows with each plan, in `PLANS` order.
fn plan_shares(sample: &Sample) -> Vec<f64> {
    PLANS
        .iter()
        .map(|p| {
            sample.rows.iter().filter(|r| r[0] == *p).count() as f64 / sample.rows.len() as f64
        })
        .collect()
}

fn assert_marginal_kept_and_dependency_lost(real: &Sample, synthetic: &Sample) {
    for (a, b) in plan_shares(real).iter().zip(plan_shares(synthetic)) {
        assert!((a - b).abs() < 0.03, "plan share {a} became {b}");
    }
    // Independent draws agree about 37% of the time (the product of the marginals), against
    // about 92% in the data.
    assert!(agreement(real) > 0.9);
    assert!(
        agreement(synthetic) < 0.45,
        "the dependency should be gone: {}",
        agreement(synthetic)
    );
}

#[test]
fn independent_marginals_keep_each_column_and_lose_the_dependency() {
    let real = plan_region(4000);
    let result = synthesize_bayes(
        &real,
        &BayesConfig {
            base: base(1),
            structure: StructureKind::Independent,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.report.method, "independent_marginals");
    assert_marginal_kept_and_dependency_lost(&real, &result.sample);
    let network = result.report.network.as_ref().unwrap();
    assert_eq!(network.structure, "independent");
    assert!(network.nodes.iter().all(|n| n.parents.is_empty()));
    assert!(result.report.limitations[0].contains("no dependency between columns is kept"));
}

#[test]
fn dp_independent_marginals_spend_the_whole_budget_on_one_histogram_per_column() {
    let real = plan_region(6000);
    let mut noise = InsecureSeededNoise::new(31);
    // structure_share is set but must be ignored: there is no structure to choose
    let config = bn_dp(4.0, StructureKind::Independent, 2, 1, dp_columns(true));
    let result = synthesize_bayes_dp(&real, &config, &mut noise).unwrap();
    assert_eq!(result.report.method, "independent_marginals_dp");
    assert_marginal_kept_and_dependency_lost(&real, &result.sample);

    let p = &result.report.privacy;
    assert_eq!(p.ledger.len(), 4, "{:?}", p.ledger);
    for spend in &p.ledger {
        assert!(spend.mechanism.starts_with("table:"), "{}", spend.mechanism);
        assert!((spend.epsilon_allotted - 1.0).abs() < 1e-12);
        assert!(spend.epsilon_used <= spend.epsilon_allotted);
    }
    assert!((p.epsilon_spent - 4.0).abs() < 1e-5, "{}", p.epsilon_spent);
    assert!(p.correlation_mechanism.starts_with("none"));
    assert!(p.statement.contains("modelled independently"));
    let network = result.report.network.as_ref().unwrap();
    assert!(network.nodes.iter().all(|n| n.parents.is_empty()));
}

#[test]
fn the_independent_marginals_config_block_takes_no_structure_settings() {
    use triplex_anonymizer::synthesize::bayesnet::dp::{config_from_declared, is_bayes};
    let block = |extra: &str| -> triplex_schemacast::DeclaredSynthesize {
        serde_json::from_str(&format!(
            r#"{{"epsilon": 1.0, "method": "independent_marginals"{extra},
                "columns": [{{"name": "plan", "type": "categorical", "values": ["free", "pro"]}}]}}"#
        ))
        .unwrap()
    };
    let plain = block("");
    assert!(is_bayes(&plain));
    assert_eq!(
        config_from_declared(&plain, None, 1).unwrap().structure,
        StructureKind::Independent
    );
    for extra in [
        r#", "structure": "chow_liu""#,
        r#", "max_parents": 2"#,
        r#", "structure_share": 0.3"#,
    ] {
        let error = config_from_declared(&block(extra), None, 1).unwrap_err();
        assert!(error.to_string().contains("has no structure"), "{error}");
    }
}

// ---- an end-to-end empirical distinguisher for the whole pipeline (a sanity check, not a proof)

/// Accuracy of the best threshold test at telling two neighboring datasets apart from one figure of
/// the synthetic output, with the structure choice, the noisy tables and the sampling all in the
/// loop. Epsilon-DP bounds it by e^eps / (1 + e^eps), whatever the figure is.
///
/// The data makes `y` follow `x`, so the replaced record moves the structure score as well as two
/// tables; the figure is the share of synthetic rows with `x = a` and `y = a`, which depends on
/// both.
fn network_distinguisher_accuracy(kind: StructureKind, epsilon: f64, trials: u64) -> f64 {
    let dataset = |aa: usize, bb: usize| Sample {
        headers: vec!["x".into(), "y".into(), "z".into()],
        rows: (0..aa + bb)
            .map(|i| {
                let v = if i < aa { "a" } else { "b" };
                let z = if i % 2 == 0 { "a" } else { "b" };
                vec![v.to_string(), v.to_string(), z.to_string()]
            })
            .collect(),
    };
    // neighbors: one record is replaced, (a, a, a) becomes (b, b, a)
    let (d0, d1) = (dataset(100, 100), {
        let mut d = dataset(100, 100);
        d.rows[0] = vec!["b".into(), "b".into(), "a".into()];
        d
    });
    let ab = |name: &str| DpColumn {
        name: name.into(),
        domain: Domain::Categorical {
            values: vec!["a".into(), "b".into()],
        },
    };
    let mut config = bn_dp(epsilon, kind, 2, 1, vec![ab("x"), ab("y"), ab("z")]);
    config.dp.rows = Some(4_000);
    let share = |d: &Sample, seed: u64| {
        let r = synthesize_bayes_dp(d, &config, &mut InsecureSeededNoise::new(seed)).unwrap();
        r.sample
            .rows
            .iter()
            .filter(|row| row[0] == "a" && row[1] == "a")
            .count() as f64
            / r.sample.rows.len() as f64
    };
    let s0: Vec<f64> = (0..trials).map(|t| share(&d0, 1000 + t)).collect();
    let s1: Vec<f64> = (0..trials).map(|t| share(&d1, 500_000 + t)).collect();
    let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
    let (m0, m1) = (mean(&s0), mean(&s1));
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

const ALL_STRUCTURES: [StructureKind; 3] = [
    StructureKind::ChowLiu,
    StructureKind::KParent,
    StructureKind::Independent,
];

#[test]
fn an_end_to_end_distinguisher_stays_within_the_dp_bound_for_every_structure() {
    let epsilon: f64 = 1.0;
    let bound = epsilon.exp() / (1.0 + epsilon.exp());
    for kind in ALL_STRUCTURES {
        let accuracy = network_distinguisher_accuracy(kind, epsilon, 400);
        assert!(
            accuracy <= bound + 0.05,
            "{kind:?}: accuracy {accuracy} exceeds the epsilon-{epsilon} bound {bound} (plus slack)"
        );
    }
}

#[test]
fn the_end_to_end_distinguisher_can_detect_leakage_when_epsilon_is_large() {
    for kind in ALL_STRUCTURES {
        let (low, high) = (
            network_distinguisher_accuracy(kind, 0.2, 300),
            network_distinguisher_accuracy(kind, 200.0, 300),
        );
        assert!(
            high > low + 0.05,
            "{kind:?}: accuracy at epsilon 200 ({high}) should clearly exceed epsilon 0.2 ({low})"
        );
    }
}
