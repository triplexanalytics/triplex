//! Only the keyed methods need a salt: bucketing runs without one, and a keyed column without
//! `salt_env` is an error that names the column.

use std::collections::HashSet;
use std::path::PathBuf;

use triplex_anonymizer::{anonymize_file, AnonymizeRecipe};

fn customers() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/sample.customers.csv")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("triplex-salt-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn bucketing_needs_no_salt_and_no_environment_variable() {
    let recipe: AnonymizeRecipe =
        serde_yaml_ng::from_str("version: 1\ncolumns:\n- name: age\n  method: bucket\n").unwrap();
    let dir = scratch("bucket");
    let report = anonymize_file(
        &customers(),
        &recipe,
        &dir.join("masked.csv"),
        &dir.join("mapping.csv"),
        None,
        &HashSet::new(),
        None,
    )
    .unwrap_or_else(|e| panic!("bucketing runs without a salt: {e}"));
    assert_eq!(report.rows_processed, 25);
    let masked = std::fs::read_to_string(dir.join("masked.csv")).unwrap();
    assert!(
        masked.lines().nth(1).unwrap().contains(",30-39,"),
        "{masked}"
    );
}

#[test]
fn a_keyed_method_without_salt_env_names_the_column() {
    let recipe: AnonymizeRecipe =
        serde_yaml_ng::from_str("version: 1\ncolumns:\n- name: email\n  method: token_hmac\n")
            .unwrap();
    let dir = scratch("keyed");
    let err = anonymize_file(
        &customers(),
        &recipe,
        &dir.join("masked.csv"),
        &dir.join("mapping.csv"),
        None,
        &HashSet::new(),
        None,
    )
    .err()
    .expect("a keyed column without salt_env fails")
    .to_string();
    assert!(err.contains("email") && err.contains("salt_env"), "{err}");
}
