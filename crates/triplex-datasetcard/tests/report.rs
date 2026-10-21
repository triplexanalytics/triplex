//! The dataset risk report on the synthetic customer fixture: each quasi-identifier hides people
//! in groups of two or more on its own, and together they single six of them out.

use std::path::PathBuf;

use triplex_datasetcard::report::{
    default_quasi_identifiers, ColumnValue, QuasiIdentifierSource, DEFAULT_BUCKET_WIDTH,
};
use triplex_datasetcard::{assess, RiskReportOptions, RowRiskModel};
use triplex_io::Sample;

fn customers() -> (Sample, triplex_schemacast::SchemaContract) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/sample.customers.csv");
    let sample = triplex_io::read_csv_full(&path).expect("fixture reads");
    let schema = triplex_schemacast::infer_schema(&sample);
    (sample, schema)
}

fn named(columns: &[&str]) -> RiskReportOptions {
    RiskReportOptions {
        quasi_identifiers: Some(columns.iter().map(|c| c.to_string()).collect()),
        ..RiskReportOptions::default()
    }
}

#[test]
fn harmless_columns_alone_single_people_out_together() {
    let (sample, schema) = customers();
    let report = assess(&sample, &schema, &named(&["age", "zip", "plan"])).unwrap();

    assert_eq!(report.rows, 25);
    assert_eq!(
        report.quasi_identifiers_chosen_by,
        QuasiIdentifierSource::Caller
    );
    let alone: Vec<(String, usize)> = report
        .columns
        .iter()
        .map(|c| (c.column.clone(), c.k))
        .collect();
    assert_eq!(
        alone,
        vec![("age".into(), 2), ("zip".into(), 5), ("plan".into(), 6)],
        "each column alone hides every person among at least two"
    );
    assert_eq!(report.combined.k, 1, "together they single someone out");
    assert_eq!(report.combined.rows_alone, 6);
    assert_eq!(report.combined.smallest_group.len(), 3);
}

#[test]
fn pairs_cover_every_combination_of_two_once() {
    let (sample, schema) = customers();
    let report = assess(&sample, &schema, &named(&["age", "zip", "plan"])).unwrap();
    let pairs: Vec<([String; 2], usize)> = report
        .pairs
        .iter()
        .map(|p| (p.columns.clone(), p.k))
        .collect();
    assert_eq!(
        pairs,
        vec![
            (["age".into(), "zip".into()], 1),
            (["age".into(), "plan".into()], 1),
            (["zip".into(), "plan".into()], 1),
        ]
    );
}

#[test]
fn numbers_are_counted_in_ranges_of_the_bucket_width() {
    let (sample, schema) = customers();
    let report = assess(&sample, &schema, &named(&["age"])).unwrap();
    assert_eq!(report.bucket_width, DEFAULT_BUCKET_WIDTH);
    // Every age value in a group label is a "lo-hi" range, never an exact age.
    let value = &report.row_risk.top[0].values[0].value;
    assert!(value.contains('-') && value.ends_with('9'), "got {value}");

    let wide = RiskReportOptions {
        bucket_width: 100,
        ..named(&["age"])
    };
    let report = assess(&sample, &schema, &wide).unwrap();
    assert_eq!(
        report.combined.k, 25,
        "one range of 100 years holds everyone"
    );
}

#[test]
fn the_riskiest_rows_come_first_with_shares_of_the_whole() {
    let (sample, schema) = customers();
    let options = RiskReportOptions {
        top_rows: 25,
        ..named(&["age", "zip", "plan"])
    };
    let report = assess(&sample, &schema, &options).unwrap();
    let top = &report.row_risk.top;
    assert_eq!(report.row_risk.model, RowRiskModel::Suda);
    assert_eq!(top.len(), 25);
    assert!(top.windows(2).all(|w| w[0].score >= w[1].score));
    let total: f64 = top.iter().map(|r| r.share).sum();
    assert!(
        (total - 1.0).abs() < 1e-9,
        "shares add up to 1, got {total}"
    );
    assert!(top.iter().all(|r| (1..=25).contains(&r.row)));
}

#[test]
fn ebm_is_available_as_the_row_model() {
    let (sample, schema) = customers();
    let options = RiskReportOptions {
        row_model: RowRiskModel::Ebm,
        ..named(&["age", "zip", "plan"])
    };
    let report = assess(&sample, &schema, &options).unwrap();
    assert_eq!(report.row_risk.model, RowRiskModel::Ebm);
    assert_eq!(report.row_risk.top.len(), 5);
}

#[test]
fn chosen_by_default_identifiers_and_decimals_are_left_out_with_a_reason() {
    let (sample, schema) = customers();
    let (chosen, excluded) = default_quasi_identifiers(&schema);
    assert_eq!(chosen, vec!["zip", "plan", "signup_date"]);
    let reasons: Vec<(&str, &str)> = excluded
        .iter()
        .map(|e| (e.column.as_str(), e.reason.as_str()))
        .collect();
    assert_eq!(
        reasons,
        vec![
            (
                "customer_id",
                "detected as an identifier: almost every value is distinct"
            ),
            (
                "age",
                "detected as an identifier: almost every value is distinct"
            ),
            (
                "monthly_spend",
                "a decimal column; name it explicitly to include it"
            ),
            ("email", "detected as Email, a direct identifier"),
        ]
    );

    let report = assess(&sample, &schema, &RiskReportOptions::default()).unwrap();
    assert_eq!(
        report.quasi_identifiers_chosen_by,
        QuasiIdentifierSource::Default
    );
    assert_eq!(report.quasi_identifiers, chosen);
    assert_eq!(report.excluded_columns, excluded);
}

#[test]
fn the_same_table_gives_the_same_report() {
    let (sample, schema) = customers();
    let options = named(&["age", "zip", "plan"]);
    let a = serde_json::to_string(&assess(&sample, &schema, &options).unwrap()).unwrap();
    let b = serde_json::to_string(&assess(&sample, &schema, &options).unwrap()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn an_unknown_column_is_an_error_not_a_guess() {
    let (sample, schema) = customers();
    let err = assess(&sample, &schema, &named(&["age", "postcode"])).unwrap_err();
    assert!(err.to_string().contains("postcode"), "got {err}");
}

#[test]
fn an_empty_table_is_an_error() {
    let (_, schema) = customers();
    let empty = Sample {
        headers: vec!["age".into()],
        rows: vec![],
    };
    assert!(assess(&empty, &schema, &named(&["age"])).is_err());
}

#[test]
fn group_values_name_their_columns() {
    let (sample, schema) = customers();
    let report = assess(&sample, &schema, &named(&["zip", "plan"])).unwrap();
    let columns: Vec<&str> = report
        .combined
        .smallest_group
        .iter()
        .map(|v: &ColumnValue| v.column.as_str())
        .collect();
    assert_eq!(columns, vec!["zip", "plan"]);
}
