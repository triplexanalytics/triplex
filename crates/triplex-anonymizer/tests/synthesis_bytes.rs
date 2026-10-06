//! The byte-slice entry points the browser uses (no filesystem): `synthesize_bytes`,
//! `synthesize_dp_bytes` and `suggest_domains` on a parsed table.

use triplex_anonymizer::synthesize::dp::{
    config_from_declared, suggest_domains, synthesize_dp_bytes, InsecureSeededNoise,
    SuggestedDomain,
};
use triplex_anonymizer::synthesize::{synthesize_bytes, SynthesizeConfig};
use triplex_schemacast::{parse_contract, read_full_bytes};

fn csv(rows: usize) -> Vec<u8> {
    let mut out = String::from("age,income,plan,note\n");
    for i in 0..rows {
        let age = 25 + (i * 7) % 40;
        out.push_str(&format!(
            "{age},{},{},row-note-{i}\n",
            20000 + age * 900 + (i % 13) * 350,
            ["free", "plus", "pro"][i % 3]
        ));
    }
    out.into_bytes()
}

fn declared(epsilon: &str) -> triplex_schemacast::DeclaredSynthesize {
    let yaml = format!("version: 1\ncolumns: []\nsynthesize:\n  epsilon: {epsilon}\n  columns:\n    - {{name: age, type: numeric, min: 18, max: 90, integer: true}}\n    - {{name: income, type: numeric, min: 0, max: 200000}}\n    - {{name: plan, type: categorical, values: [free, plus, pro]}}\n");
    parse_contract(&yaml).unwrap().synthesize.unwrap()
}

#[test]
fn synthesize_bytes_returns_a_readable_csv_and_a_report() {
    let (csv_bytes, report) =
        synthesize_bytes(&csv(300), "csv", &SynthesizeConfig::default()).unwrap();
    let back = read_full_bytes(&csv_bytes, "csv").unwrap();
    assert_eq!(back.headers, report.columns_synthesized);
    assert_eq!(back.rows.len(), 300);
    assert!(
        report.columns_dropped.iter().any(|d| d.column == "note"),
        "a unique-string column is dropped"
    );
}

#[test]
fn synthesize_bytes_matches_the_file_based_result_for_the_same_seed() {
    let dir = std::env::temp_dir().join("triplex_synth_bytes_parity");
    std::fs::create_dir_all(&dir).unwrap();
    let (input, output) = (dir.join("in.csv"), dir.join("out.csv"));
    std::fs::write(&input, csv(200)).unwrap();
    let config = SynthesizeConfig {
        seed: 9,
        ..Default::default()
    };
    triplex_anonymizer::synthesize::synthesize_file(&input, &output, &config).unwrap();
    let (bytes_csv, _) = synthesize_bytes(&csv(200), "csv", &config).unwrap();
    assert_eq!(std::fs::read(&output).unwrap(), bytes_csv);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unsupported_extension_is_a_clear_error() {
    let e = synthesize_bytes(b"a,b\n1,2\n", "parquet", &SynthesizeConfig::default())
        .err()
        .unwrap()
        .to_string();
    assert!(e.contains("parquet"), "{e}");
}

#[test]
fn synthesize_dp_bytes_respects_the_budget_and_omits_diagnostics_by_default() {
    let config = config_from_declared(&declared("2.0"), Some(120), 3).unwrap();
    let (csv_bytes, report) = synthesize_dp_bytes(
        &csv(300),
        "csv",
        &config,
        false,
        &mut InsecureSeededNoise::new(1),
    )
    .unwrap();
    assert_eq!(read_full_bytes(&csv_bytes, "csv").unwrap().rows.len(), 120);
    assert!(report.nonprivate_diagnostics.is_none());
    assert!(report.privacy.epsilon_spent <= 2.0 * (1.0 + 1e-9));
    let (_, with) = synthesize_dp_bytes(
        &csv(300),
        "csv",
        &config,
        true,
        &mut InsecureSeededNoise::new(1),
    )
    .unwrap();
    assert!(with.nonprivate_diagnostics.is_some());
}

#[test]
fn the_dp_config_json_the_browser_sends_parses_like_the_yaml_block() {
    // the wasm export deserializes the same struct from JSON
    let json = r#"{"epsilon": 1.5, "columns": [{"name": "age", "type": "numeric", "min": 18, "max": 90, "integer": true}, {"name": "plan", "type": "categorical", "values": ["free", "pro"]}]}"#;
    let d: triplex_schemacast::DeclaredSynthesize = serde_json::from_str(json).unwrap();
    let c = config_from_declared(&d, None, 1).unwrap();
    assert_eq!((c.epsilon, c.columns.len()), (1.5, 2));
    let missing: triplex_schemacast::DeclaredSynthesize =
        serde_json::from_str(r#"{"columns": []}"#).unwrap();
    assert!(config_from_declared(&missing, None, 1)
        .err()
        .unwrap()
        .to_string()
        .contains("no default"));
}

#[test]
fn suggestions_come_from_the_parsed_bytes_and_flag_the_identifier_column() {
    let sample = read_full_bytes(&csv(300), "csv").unwrap();
    let s = suggest_domains(&sample);
    assert_eq!(s.len(), 4);
    assert!(matches!(
        s[0].domain,
        SuggestedDomain::Numeric { integer: true, .. }
    ));
    assert!(s[2].suggested_include);
    assert!(
        !s[3].suggested_include && s[3].domain == SuggestedDomain::None,
        "the unique note column"
    );
}

// ---- Bayesian network byte APIs

#[test]
fn bayes_bytes_round_trips_through_csv_and_reports_the_network() {
    use triplex_anonymizer::synthesize::bayesnet::{synthesize_bayes_bytes, BayesConfig};
    let mut csv = String::from("plan,region\n");
    for i in 0..300 {
        let (plan, region) = if i % 2 == 0 {
            ("free", "north")
        } else {
            ("pro", "south")
        };
        csv.push_str(&format!("{plan},{region}\n"));
    }
    let (out, report) =
        synthesize_bayes_bytes(csv.as_bytes(), "csv", &BayesConfig::default()).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 301);
    assert!(text.starts_with("plan,region\n"));
    assert_eq!(report.network.unwrap().nodes.len(), 2);
}

#[test]
fn bayes_dp_bytes_with_and_without_diagnostics() {
    use triplex_anonymizer::synthesize::bayesnet::dp::{
        synthesize_bayes_dp_bytes, BayesDpConfig, DEFAULT_STRUCTURE_SHARE,
    };
    use triplex_anonymizer::synthesize::bayesnet::StructureKind;
    use triplex_anonymizer::synthesize::dp::{Domain, DpColumn, DpConfig, InsecureSeededNoise};
    let mut csv = String::from("plan,region\n");
    for i in 0..400 {
        let (plan, region) = if i % 2 == 0 {
            ("free", "north")
        } else {
            ("pro", "south")
        };
        csv.push_str(&format!("{plan},{region}\n"));
    }
    let cat = |name: &str, values: &[&str]| DpColumn {
        name: name.into(),
        domain: Domain::Categorical {
            values: values.iter().map(|v| v.to_string()).collect(),
        },
    };
    let config = BayesDpConfig {
        dp: DpConfig {
            epsilon: 20.0,
            marginal_share: 0.5,
            columns: vec![
                cat("plan", &["free", "pro"]),
                cat("region", &["north", "south"]),
            ],
            rows: None,
            sampling_seed: 1,
            exclude_rows: Default::default(),
        },
        structure: StructureKind::ChowLiu,
        max_parents: 1,
        structure_share: DEFAULT_STRUCTURE_SHARE,
    };
    let mut noise = InsecureSeededNoise::new(1);
    let (out, report) =
        synthesize_bayes_dp_bytes(csv.as_bytes(), "csv", &config, false, &mut noise).unwrap();
    assert_eq!(String::from_utf8(out).unwrap().lines().count(), 401);
    assert!(report.nonprivate_diagnostics.is_none());
    let mut noise = InsecureSeededNoise::new(1);
    let (_, with) =
        synthesize_bayes_dp_bytes(csv.as_bytes(), "csv", &config, true, &mut noise).unwrap();
    assert!(with.nonprivate_diagnostics.is_some());
}
