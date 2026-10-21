//! `triplex risk` end to end: the real binary on the synthetic customer fixture.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn risk(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_triplex"))
        .arg("risk")
        .args(args)
        .output()
        .expect("triplex runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn reports_the_combined_risk_as_json() {
    let input = fixture("sample.customers.csv");
    let (code, stdout, _) = risk(&[
        "--input",
        input.to_str().unwrap(),
        "--quasi",
        "age,zip,plan",
    ]);
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["combined"]["k"], 1);
    assert_eq!(report["combined"]["rows_alone"], 6);
    assert_eq!(report["columns"][2]["k"], 6);
    assert_eq!(report["row_risk"]["model"], "suda");
}

#[test]
fn without_quasi_it_chooses_and_says_what_it_left_out() {
    let input = fixture("sample.customers.csv");
    let (code, stdout, _) = risk(&["--input", input.to_str().unwrap()]);
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["quasi_identifiers_chosen_by"], "default");
    let left_out: Vec<&str> = report["excluded_columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["column"].as_str().unwrap())
        .collect();
    assert!(left_out.contains(&"email"));
}

#[test]
fn bad_input_is_an_error_with_a_message() {
    let input = fixture("sample.customers.csv");
    let (code, _, stderr) = risk(&["--input", input.to_str().unwrap(), "--quasi", "postcode"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("postcode"), "got {stderr}");

    let (code, _, stderr) = risk(&["--input", input.to_str().unwrap(), "--rows", "magic"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("suda or ebm"), "got {stderr}");
}
