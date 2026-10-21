//! A contract drafted from data is written without empty entries: a part that was not set is
//! left out, not written as `null`. Contracts written the old way, with `null`s, still read.

use std::path::PathBuf;

fn customers() -> Vec<u8> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/sample.customers.csv");
    std::fs::read(path).unwrap()
}

#[test]
fn a_drafted_contract_has_no_null_entries() {
    let yaml = triplex_schemacast::generate_contract_yaml_from_bytes(&customers(), "csv").unwrap();
    assert!(!yaml.contains("null"), "{yaml}");
    assert!(
        yaml.contains("name: email") && yaml.contains("type: string"),
        "{yaml}"
    );

    let back: triplex_schemacast::DeclaredContract = serde_yaml_ng::from_str(&yaml).unwrap();
    assert_eq!(serde_yaml_ng::to_string(&back).unwrap(), yaml);
}

#[test]
fn a_contract_with_null_entries_still_reads() {
    let old = "version: 1\ncolumns:\n- name: email\n  type: string\n  required: true\n  \
               statistics: null\n  pattern: null\ndriftguard: null\nmodelcard: null\nsynthesize: null\n";
    let contract: triplex_schemacast::DeclaredContract = serde_yaml_ng::from_str(old).unwrap();
    assert_eq!(contract.columns.len(), 1);
    assert!(!serde_yaml_ng::to_string(&contract)
        .unwrap()
        .contains("null"));
}
