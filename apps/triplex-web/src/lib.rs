//! Thin `wasm-bindgen` surface over SchemaCast/Anonymizer for the browser.
//! Every action here runs entirely client-side: input bytes come from a
//! file the user picked, output bytes are handed back to the page for a
//! browser download. No network call, no server — see
//! the design notes's S3.5 step.
//!
//! Mirrors `apps/triplex-cli`'s role for the native binary: this crate owns
//! JS-facing signatures and error conversion only, no logic of its own —
//! everything real lives in `triplex-schemacast`/`triplex-anonymizer`.

use std::collections::HashMap;
use triplex_anonymizer::{AnonymizeMethod, AnonymizeRecipe};
use triplex_schemacast::DeclaredContract;
use wasm_bindgen::prelude::*;

fn to_js_err<E: std::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Table view's result: the per-column profile (type, null%, mismatched%,
/// quantiles/histogram or top-value counts — small, stays JSON) and the
/// whole uploaded file's row data as Arrow IPC bytes (typed columns, not a
/// JSON string of every stringified cell — unlike `generate_contract`'s
/// schema inference, the row data isn't bounded to a sample, since the
/// browser already holds the whole upload in memory).
#[wasm_bindgen(getter_with_clone)]
pub struct TableAndProfile {
    pub profile_json: String,
    pub table_arrow: Vec<u8>,
}

/// Table view: parses `bytes` once and returns both the schema profile (for
/// the header sparklines and Column-view tab) and the row data, replacing
/// the old `read_table`+`profile_columns` pair — each of which independently
/// re-parsed the whole file. See the design notes.
#[wasm_bindgen]
pub fn read_and_profile_table(bytes: &[u8], extension: &str) -> Result<TableAndProfile, JsValue> {
    let (schema, table_arrow) =
        triplex_schemacast::read_and_profile_bytes(bytes, extension).map_err(to_js_err)?;
    let profile_json = serde_json::to_string(&schema).map_err(to_js_err)?;
    Ok(TableAndProfile {
        profile_json,
        table_arrow,
    })
}

/// Table view: every anonymization method this build implements, as a JSON
/// array of their `.triplex.yaml` names (e.g. `["token_hmac"]") — the
/// per-column algorithm picker reads this instead of hardcoding a menu that
/// can drift out of sync with `triplex-anonymizer`.
#[wasm_bindgen]
pub fn list_anonymize_methods() -> String {
    serde_json::to_string(AnonymizeMethod::ALL).unwrap_or_else(|_| "[]".to_string())
}

/// Table view: renders a contract built up from table interactions (as JSON
/// matching `DeclaredContract`'s shape) into `.triplex.yaml` text for the
/// navbar's "Download contract" button — Rust stays the one place that knows
/// the YAML shape, not a JS YAML library. Verified well-formed first (e.g.
/// every declared `pattern` actually compiles) so a broken contract fails
/// here, immediately, rather than only when it's later used to check
/// pattern conformance.
#[wasm_bindgen]
pub fn contract_json_to_yaml(contract_json: &str) -> Result<String, JsValue> {
    let contract: DeclaredContract = serde_json::from_str(contract_json).map_err(to_js_err)?;
    triplex_schemacast::verify_contract_well_formed(&contract).map_err(to_js_err)?;
    serde_yaml_ng::to_string(&contract).map_err(to_js_err)
}

/// Table view: same as `contract_json_to_yaml`, for the accumulated
/// anonymization recipe and the "Download recipe" button — verified
/// well-formed first (at least one column, no empty column names).
#[wasm_bindgen]
pub fn recipe_json_to_yaml(recipe_json: &str) -> Result<String, JsValue> {
    let recipe: AnonymizeRecipe = serde_json::from_str(recipe_json).map_err(to_js_err)?;
    triplex_anonymizer::verify_recipe_well_formed(&recipe).map_err(to_js_err)?;
    serde_yaml_ng::to_string(&recipe).map_err(to_js_err)
}

/// Action 1: infers a schema from an uploaded file's bytes and renders it
/// as a starting `.triplex.yaml` contract the user can download and edit.
/// `extension` is the file's extension without a leading dot (`"csv"`,
/// `"tsv"`, or `"jsonl"` — `.parquet` isn't supported in the browser).
#[wasm_bindgen]
pub fn generate_contract(bytes: &[u8], extension: &str) -> Result<String, JsValue> {
    triplex_schemacast::generate_contract_yaml_from_bytes(bytes, extension).map_err(to_js_err)
}

/// Lists the column names declared in an anonymize recipe's YAML text, as a
/// JSON array — lets the page render one salt input per column without
/// needing a YAML parser in JavaScript.
#[wasm_bindgen]
pub fn list_recipe_columns(recipe_yaml: &str) -> Result<String, JsValue> {
    let recipe: AnonymizeRecipe = serde_yaml_ng::from_str(recipe_yaml).map_err(to_js_err)?;
    let names: Vec<&str> = recipe.columns.iter().map(|c| c.name.as_str()).collect();
    serde_json::to_string(&names).map_err(to_js_err)
}

/// Action 2's result: the masked dataset and its reversible mapping, both
/// as downloadable CSV bytes, plus the run report as a JSON string.
#[wasm_bindgen(getter_with_clone)]
pub struct AnonymizeOutput {
    pub masked_csv: Vec<u8>,
    pub mapping_csv: Vec<u8>,
    pub report_json: String,
}

/// Action 2: runs Tier 1 (HMAC) anonymization against an uploaded file's
/// bytes. `recipe_yaml` is a `.triplex.yaml` anonymize recipe's text;
/// `salts_json` is a JSON object mapping each declared column name to the
/// salt the user typed into the page (there's no OS environment to read a
/// `salt_env` from in a browser — see `triplex_anonymizer::anonymize_bytes`).
/// The mapping file is always plaintext here (no passphrase-encryption
/// option in the browser flow) — still never leaves this page.
#[wasm_bindgen]
pub fn anonymize(
    bytes: &[u8],
    extension: &str,
    recipe_yaml: &str,
    salts_json: &str,
) -> Result<AnonymizeOutput, JsValue> {
    let recipe: AnonymizeRecipe = serde_yaml_ng::from_str(recipe_yaml).map_err(to_js_err)?;
    let salts: HashMap<String, String> = serde_json::from_str(salts_json).map_err(to_js_err)?;

    let result =
        triplex_anonymizer::anonymize_bytes(bytes, extension, &recipe, &salts).map_err(to_js_err)?;
    let report_json = serde_json::to_string(&result.report).map_err(to_js_err)?;

    Ok(AnonymizeOutput {
        masked_csv: result.masked_csv,
        mapping_csv: result.mapping_csv,
        report_json,
    })
}

/// Action 3: checks whether `bytes` (typically Action 2's masked output)
/// still conforms to every `pattern` declared in `contract_yaml` — the
/// original contract, from Action 1 or hand-authored. Returns the
/// `ValidationReport` as a JSON string.
#[wasm_bindgen]
pub fn check_pattern_conformance(
    bytes: &[u8],
    extension: &str,
    contract_yaml: &str,
) -> Result<String, JsValue> {
    let declared = triplex_schemacast::parse_contract(contract_yaml).map_err(to_js_err)?;
    let report =
        triplex_schemacast::check_patterns_bytes(&declared, bytes, extension).map_err(to_js_err)?;
    serde_json::to_string(&report).map_err(to_js_err)
}

/// Column-view tab: a pre-anonymize privacy-risk advisory
///  — k-anonymity for one numeric column's raw values, computed
/// straight from the upload, before the user has picked any anonymization
/// method or recipe. `values` is that column's values as a typed `f64`
/// array (every current caller pre-filters to integer-inferred columns, so
/// this is a real typed array from the browser's Arrow-backed table, not a
/// JSON string of stringified cells — see the design notes follow-up);
/// `NaN` marks a missing/non-numeric cell, bucketed into its own singleton
/// bucket same as `bucket_numeric`'s doc comment describes. Returns a
/// `RiskAssessment` as JSON. k-anonymity alone needs no second column — see
/// `score_column_pair_risk` for l-diversity/t-closeness, which do.
#[wasm_bindgen]
pub fn score_column_risk(values: &[f64], bucket_width: u32) -> Result<String, JsValue> {
    let bucketed: Vec<String> = values
        .iter()
        .map(|v| triplex_anonymizer::bucket_numeric(bucket_width as u64, *v))
        .collect();
    let assessment = triplex_datasetcard::evaluate_risk_model(
        triplex_datasetcard::RiskModel::KAnonymity,
        &bucketed,
        &[],
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&assessment).map_err(to_js_err)
}

/// Dataset Anonymity Analysis tab: l-diversity and t-closeness for one
/// user-chosen pair of columns — `quasi_values` (the quasi-identifier,
/// typed and generalized the same way as `score_column_risk`) and
/// `sensitive_values_json` (the attribute checked for diversity/skew within
/// each generalized group — a JSON array of strings, not typed, since a
/// candidate sensitive column can be any type, not just numeric), both
/// that column's raw values in row order. Returns a two-element JSON array
/// of `RiskAssessment` (`[l_diversity, t_closeness]`).
#[wasm_bindgen]
pub fn score_column_pair_risk(
    quasi_values: &[f64],
    sensitive_values_json: &str,
    bucket_width: u32,
) -> Result<String, JsValue> {
    let sensitive_values: Vec<String> =
        serde_json::from_str(sensitive_values_json).map_err(to_js_err)?;
    let bucketed: Vec<String> = quasi_values
        .iter()
        .map(|v| triplex_anonymizer::bucket_numeric(bucket_width as u64, *v))
        .collect();
    let l_diversity = triplex_datasetcard::evaluate_risk_model(
        triplex_datasetcard::RiskModel::LDiversity,
        &bucketed,
        &sensitive_values,
    )
    .map_err(to_js_err)?;
    let t_closeness = triplex_datasetcard::evaluate_risk_model(
        triplex_datasetcard::RiskModel::TCloseness,
        &bucketed,
        &sensitive_values,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&[l_diversity, t_closeness]).map_err(to_js_err)
}

/// Column-view tab: `score_column_risk`'s string-column counterpart — a
/// string/categorical column has no numeric range to generalize into a
/// `"lo-hi"` bucket, so its equivalence classes are just its exact values
/// (the same grouping `categorical_distribution`'s "Distinct" count already
/// does — see the design notes). `values_json` is
/// that column's raw values in row order, as a JSON array of strings — a
/// typed array isn't an option for strings the way `&[f64]` is for numbers.
#[wasm_bindgen]
pub fn score_column_risk_string(values_json: &str) -> Result<String, JsValue> {
    let values: Vec<String> = serde_json::from_str(values_json).map_err(to_js_err)?;
    let assessment = triplex_datasetcard::evaluate_risk_model(
        triplex_datasetcard::RiskModel::KAnonymity,
        &values,
        &[],
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&assessment).map_err(to_js_err)
}

/// Dataset Anonymity Analysis tab: `score_column_pair_risk`'s string-column
/// counterpart, for when the quasi-identifier column itself is a string —
/// see `score_column_risk_string`'s doc comment for why it's ungeneralized
/// exact-value grouping rather than a bucket width. Returns the same
/// two-element `[l_diversity, t_closeness]` JSON array.
#[wasm_bindgen]
pub fn score_column_pair_risk_string(
    quasi_values_json: &str,
    sensitive_values_json: &str,
) -> Result<String, JsValue> {
    let quasi_values: Vec<String> = serde_json::from_str(quasi_values_json).map_err(to_js_err)?;
    let sensitive_values: Vec<String> =
        serde_json::from_str(sensitive_values_json).map_err(to_js_err)?;
    let l_diversity = triplex_datasetcard::evaluate_risk_model(
        triplex_datasetcard::RiskModel::LDiversity,
        &quasi_values,
        &sensitive_values,
    )
    .map_err(to_js_err)?;
    let t_closeness = triplex_datasetcard::evaluate_risk_model(
        triplex_datasetcard::RiskModel::TCloseness,
        &quasi_values,
        &sensitive_values,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&[l_diversity, t_closeness]).map_err(to_js_err)
}

/// Generalizes one numeric column into `"lo-hi"` buckets — the same
/// generalization `score_column_risk` applies internally, exposed
/// standalone so the caller can pre-bucket several numeric columns and
/// combine them with string columns before scoring their *combined*
/// quasi-identifier (see `score_joint_column_risk`). A string column needs
/// no such step: it's already scored on exact values.
#[wasm_bindgen]
pub fn bucket_numeric_column(values: &[f64], bucket_width: u32) -> Vec<String> {
    values
        .iter()
        .map(|v| triplex_anonymizer::bucket_numeric(bucket_width as u64, *v))
        .collect()
}

/// Dataset Anonymity Analysis tab: k-anonymity for a *combined*
/// quasi-identifier — several columns' worth of already-bucketed/raw
/// values (numeric columns pre-bucketed via `bucket_numeric_column`,
/// string columns passed as their exact values), zipped row-for-row into
/// one equivalence-class key per row. This is the real dataset-level
/// verdict a single column's score can't give: two columns that each look
/// safe scored alone can combine into a unique fingerprint per row (see
/// `triplex_datasetcard::compose_quasi_identifiers`'s doc comment).
/// `columns_json` is a JSON array of arrays of strings, one inner array
/// per column, same row order and length. Returns a `RiskAssessment` as
/// JSON, same shape as `score_column_risk`.
#[wasm_bindgen]
pub fn score_joint_column_risk(columns_json: &str) -> Result<String, JsValue> {
    let columns: Vec<Vec<String>> = serde_json::from_str(columns_json).map_err(to_js_err)?;
    let composite = triplex_datasetcard::compose_quasi_identifiers(&columns);
    let assessment = triplex_datasetcard::evaluate_risk_model(
        triplex_datasetcard::RiskModel::KAnonymity,
        &composite,
        &[],
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&assessment).map_err(to_js_err)
}

/// `score_joint_column_risk`'s l-diversity/t-closeness counterpart, mirroring
/// `score_column_pair_risk`'s shape: the same combined quasi-identifier
/// (`columns_json`, see `score_joint_column_risk`) checked against one
/// candidate sensitive column (`sensitive_values_json`, that column's raw
/// values in row order). Returns the same two-element
/// `[l_diversity, t_closeness]` JSON array.
#[wasm_bindgen]
pub fn score_joint_column_pair_risk(
    columns_json: &str,
    sensitive_values_json: &str,
) -> Result<String, JsValue> {
    let columns: Vec<Vec<String>> = serde_json::from_str(columns_json).map_err(to_js_err)?;
    let sensitive_values: Vec<String> =
        serde_json::from_str(sensitive_values_json).map_err(to_js_err)?;
    let composite = triplex_datasetcard::compose_quasi_identifiers(&columns);
    let l_diversity = triplex_datasetcard::evaluate_risk_model(
        triplex_datasetcard::RiskModel::LDiversity,
        &composite,
        &sensitive_values,
    )
    .map_err(to_js_err)?;
    let t_closeness = triplex_datasetcard::evaluate_risk_model(
        triplex_datasetcard::RiskModel::TCloseness,
        &composite,
        &sensitive_values,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&[l_diversity, t_closeness]).map_err(to_js_err)
}
