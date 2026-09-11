//! DatasetCard: dataset privacy-risk scoring at the dataset/column/record
//! level, independent of any anonymization recipe — see
//! the design notes and
//! the design notes for the product decision.
//!
//! Column-level scoring, extracted from `crates/triplex-anonymizer/src/
//! bucket.rs`'s original implementation (same functions, same tests, no
//! behavior change): the named formal privacy models a bucketed/
//! generalized column can be checked against. `compose_quasi_identifiers`
//! builds dataset-level scoring on top of the same functions, run against
//! a combined quasi-identifier instead of one column. Record-level
//! (SUDA-style) scoring is still a later phase — see the the design notes
//! entry for what's deliberately not built yet.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use triplex_core::{Error, Result};

/// A named formal-privacy model a bucketed (generalized) column can be
/// checked against — see the design notes's Tier 1 row.
/// `evaluate_risk_model` computes the first three directly from values
/// already in the dataset. δ-presence is different in kind: it bounds the
/// probability that an individual is present in the dataset *at all*,
/// which needs a reference population table strictly larger than this
/// dataset — a concept nothing in this crate has yet.
/// ponytail: δ-presence named but unimplemented; add a population-table
/// type and a real arm here once one exists, rather than fake a result now
/// — see the project guidelines's rule that a privacy claim is never asserted without a
/// passing test backing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskModel {
    KAnonymity,
    LDiversity,
    TCloseness,
    DeltaPresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "model", content = "value", rename_all = "snake_case")]
pub enum RiskAssessment {
    KAnonymity(usize),
    LDiversity(usize),
    TCloseness(f64),
}

/// The size of the smallest equivalence class in `bucketed_values` — every
/// row sharing a bucket value is indistinguishable from the rest of its
/// class on this column alone.
pub fn k_anonymity(bucketed_values: &[String]) -> usize {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for v in bucketed_values {
        *counts.entry(v.as_str()).or_insert(0) += 1;
    }
    counts.values().copied().min().unwrap_or(0)
}

/// The smallest number of distinct sensitive-attribute values found in any
/// one equivalence class. `sensitive_values` must be aligned row-for-row
/// with `bucketed_values` (same rows, same order, unmasked).
pub fn l_diversity(bucketed_values: &[String], sensitive_values: &[String]) -> usize {
    let mut classes: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (bucket, sensitive) in bucketed_values.iter().zip(sensitive_values) {
        classes
            .entry(bucket.as_str())
            .or_default()
            .insert(sensitive.as_str());
    }
    classes.values().map(|s| s.len()).min().unwrap_or(0)
}

fn distribution<'a>(values: &[&'a str]) -> HashMap<&'a str, f64> {
    let mut counts: HashMap<&str, f64> = HashMap::new();
    for v in values {
        *counts.entry(v).or_insert(0.0) += 1.0;
    }
    let total = values.len() as f64;
    for count in counts.values_mut() {
        *count /= total;
    }
    counts
}

/// Total variation distance between two categorical distributions: half the
/// sum of the absolute difference in probability mass over every category
/// either one puts weight on. 0.0 means identical distributions; 1.0 means
/// they share no probability mass at all.
///
/// `a` is `t_closeness`'s whole-dataset distribution (as many distinct
/// categories as the sensitive column has — can be huge, e.g. a
/// near-unique numeric column) and `b` is one equivalence class's local
/// distribution (typically tiny — one bucket's worth of rows). Iterating
/// the *union* of both key sets costs O(|a|) per call regardless of how
/// small `b` is, and `t_closeness` calls this once per bucket — O(buckets
/// x |a|) in total, which is billions of hashmap lookups for a dataset
/// with tens of thousands of buckets and a high-cardinality sensitive
/// column (confirmed hanging in practice before this fix). Since `a`'s
/// probabilities already sum to 1.0, the mass contributed by categories in
/// `a` but not `b` is just `1.0 - (a's mass restricted to b's keys)` — no
/// need to visit those categories individually, so this only ever costs
/// O(|b|).
fn total_variation_distance(a: &HashMap<&str, f64>, b: &HashMap<&str, f64>) -> f64 {
    let mut sum_over_b_keys = 0.0;
    let mut a_mass_within_b = 0.0;
    for (category, b_share) in b {
        let a_share = a.get(category).copied().unwrap_or(0.0);
        sum_over_b_keys += (a_share - b_share).abs();
        a_mass_within_b += a_share;
    }
    let a_mass_outside_b = (1.0 - a_mass_within_b).max(0.0);
    (sum_over_b_keys + a_mass_outside_b) / 2.0
}

/// The largest total-variation distance between any one equivalence
/// class's sensitive-attribute distribution and that attribute's
/// distribution across the whole dataset — 0.0 (tightest) means every
/// class matches the global distribution exactly; 1.0 means some class
/// shares no sensitive value at all with the global distribution.
pub fn t_closeness(bucketed_values: &[String], sensitive_values: &[String]) -> f64 {
    let all_sensitive: Vec<&str> = sensitive_values.iter().map(String::as_str).collect();
    let overall = distribution(&all_sensitive);

    let mut classes: HashMap<&str, Vec<&str>> = HashMap::new();
    for (bucket, sensitive) in bucketed_values.iter().zip(sensitive_values) {
        classes
            .entry(bucket.as_str())
            .or_default()
            .push(sensitive.as_str());
    }

    classes
        .values()
        .map(|class_values| total_variation_distance(&overall, &distribution(class_values)))
        .fold(0.0, f64::max)
}

/// Zips N already-bucketed/generalized columns into one composite
/// equivalence-class key per row, so `k_anonymity`/`l_diversity`/
/// `t_closeness` can be run against the *combined* quasi-identifier
/// instead of one column at a time. This is what k-anonymity is actually
/// defined over: two columns can each look safe scored alone (e.g. a ZIP
/// code shared by thousands of people, a birth year shared by millions)
/// while their combination re-identifies almost everyone, because the
/// equivalence classes that matter are the joint ones — see
/// the design notes's "dataset-level
/// aggregation method" gap. Row `i`'s key is every column's row-`i` value
/// joined by U+001F (Unit Separator), a control character that can't
/// appear in a numeric `"lo-hi"` bucket label or ordinary CSV text.
///
/// Rows are matched positionally; if columns disagree on length the
/// result truncates to the shortest, the same "trust row alignment,
/// degrade gracefully rather than panic" behavior `l_diversity`/
/// `t_closeness` already have via their own `zip`.
pub fn compose_quasi_identifiers(columns: &[Vec<String>]) -> Vec<String> {
    let mut composite: Vec<String> = match columns.first() {
        Some(first) => first.clone(),
        None => return Vec::new(),
    };
    for column in &columns[1..] {
        for (key, value) in composite.iter_mut().zip(column) {
            key.push('\u{1f}');
            key.push_str(value);
        }
        composite.truncate(column.len().min(composite.len()));
    }
    composite
}

/// Dispatches to the risk model named by `model`. Exhaustive: a fifth
/// `RiskModel` variant forces a new arm here instead of silently falling
/// through to a wrong or fabricated result.
pub fn evaluate_risk_model(
    model: RiskModel,
    bucketed_values: &[String],
    sensitive_values: &[String],
) -> Result<RiskAssessment> {
    match model {
        RiskModel::KAnonymity => Ok(RiskAssessment::KAnonymity(k_anonymity(bucketed_values))),
        RiskModel::LDiversity => Ok(RiskAssessment::LDiversity(l_diversity(
            bucketed_values,
            sensitive_values,
        ))),
        RiskModel::TCloseness => Ok(RiskAssessment::TCloseness(t_closeness(
            bucketed_values,
            sensitive_values,
        ))),
        RiskModel::DeltaPresence => Err(Error::Validation(
            "delta-presence needs a reference population table, which isn't implemented yet"
                .to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn k_anonymity_is_the_smallest_equivalence_class_size() {
        let buckets = vec!["20-29", "20-29", "20-29", "30-39", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(k_anonymity(&buckets), 2); // "30-39" has only 2 rows
    }

    #[test]
    fn l_diversity_is_the_fewest_distinct_sensitive_values_in_any_class() {
        let buckets = vec!["20-29", "20-29", "20-29", "30-39", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = vec!["flu", "flu", "flu", "flu", "diabetes"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        // "20-29" only ever has "flu" (1 distinct); "30-39" has "flu" and
        // "diabetes" (2 distinct) — the minimum across classes is 1.
        assert_eq!(l_diversity(&buckets, &diagnoses), 1);
    }

    #[test]
    fn t_closeness_is_zero_when_every_class_matches_the_global_distribution() {
        let buckets = vec!["20-29", "20-29", "30-39", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = vec!["flu", "diabetes", "flu", "diabetes"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(t_closeness(&buckets, &diagnoses), 0.0);
    }

    #[test]
    fn t_closeness_is_high_when_a_class_diverges_from_the_global_distribution() {
        let buckets = vec!["20-29", "20-29", "20-29", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = vec!["flu", "flu", "flu", "diabetes"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        // "20-29" is 100% "flu" while the global mix is 75%/25% — a real gap.
        assert!(t_closeness(&buckets, &diagnoses) > 0.0);
    }

    /// Exercises exactly the case the optimized `total_variation_distance`
    /// must still get right: a sensitive category present in the overall
    /// distribution but completely absent from one class's local
    /// distribution. That category's whole probability mass has to count
    /// toward the distance even though the optimized path never visits it
    /// by name — it falls out of the `1.0 - a_mass_within_b` residual term
    /// instead of an explicit per-category lookup.
    #[test]
    fn t_closeness_counts_a_category_missing_from_a_class_entirely() {
        // Global mix: flu 1/2, diabetes 1/4, asthma 1/4. The "20-29" class
        // is 100% flu — diabetes and asthma both contribute their full
        // global share to the distance, not just diabetes (a category that
        // does appear elsewhere in "30-39").
        let buckets = vec!["20-29", "20-29", "30-39", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = vec!["flu", "flu", "flu", "diabetes"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        // Global: flu 3/4, diabetes 1/4. "20-29" class: flu 1.0.
        // TVD = 0.5 * (|1.0 - 0.75| + |0.0 - 0.25|) = 0.25.
        assert_eq!(t_closeness(&buckets, &diagnoses), 0.25);
    }

    /// A synthetic worst case that hung in practice before this fix: an
    /// id-like quasi-identifier bucketed at a narrow width produces
    /// thousands of small buckets, checked against a sensitive column with
    /// high cardinality (tens of thousands of distinct values) — exactly
    /// the shape a real "which column looks risky" scan hits when a
    /// dataset includes a near-unique numeric column. The old
    /// implementation was O(buckets x overall_cardinality); this bounds
    /// wall-clock time the same way `triplex-schemacast`'s existing
    /// 100k-row inference test does, rather than asserting a specific
    /// number that would be meaningless across machines.
    #[test]
    fn t_closeness_stays_fast_with_many_buckets_and_a_high_cardinality_sensitive_column() {
        use std::time::{Duration, Instant};

        let rows = 50_000;
        let bucket_width = 10;
        let buckets: Vec<String> = (0..rows)
            .map(|i: usize| {
                let lo = (i / bucket_width) * bucket_width;
                format!("{lo}-{}", lo + bucket_width - 1)
            })
            .collect();
        // Each row gets its own distinct sensitive value — the pathological
        // high-cardinality case.
        let sensitive: Vec<String> = (0..rows).map(|i| format!("value-{i}")).collect();

        let start = Instant::now();
        let result = t_closeness(&buckets, &sensitive);
        let elapsed = start.elapsed();

        assert!((0.0..=1.0).contains(&result));
        assert!(
            elapsed < Duration::from_secs(2),
            "t_closeness took {elapsed:?} for {rows} rows / {bucket_width}-wide buckets \
             against an all-distinct sensitive column — expected well under 2s"
        );
    }

    #[test]
    fn compose_quasi_identifiers_combines_columns_into_one_key_per_row() {
        let zip = vec!["02138", "02138", "02139"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let birth_year = vec!["1990", "1990", "1990"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let composite = compose_quasi_identifiers(&[zip, birth_year]);
        assert_eq!(composite.len(), 3);
        assert_eq!(composite[0], composite[1]); // same zip, same birth year
        assert_ne!(composite[0], composite[2]); // different zip
    }

    /// The whole reason this function exists: two columns that each look
    /// safe scored independently can combine into a unique fingerprint per
    /// row. `k_anonymity` on the composite must see that drop, not just
    /// repeat whichever single-column score was already known.
    #[test]
    fn joint_k_anonymity_can_be_lower_than_either_column_alone() {
        let group = vec!["X", "X", "Y", "Y"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let parity = vec!["odd", "even", "odd", "even"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(k_anonymity(&group), 2);
        assert_eq!(k_anonymity(&parity), 2);

        let composite = compose_quasi_identifiers(&[group, parity]);
        assert_eq!(k_anonymity(&composite), 1); // every (group, parity) pair is unique
    }

    #[test]
    fn compose_quasi_identifiers_is_empty_for_no_columns() {
        assert_eq!(compose_quasi_identifiers(&[]), Vec::<String>::new());
    }

    #[test]
    fn compose_quasi_identifiers_truncates_to_the_shortest_column() {
        let a = vec!["X", "X", "Y"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let b = vec!["1", "2"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(compose_quasi_identifiers(&[a, b]).len(), 2);
    }

    #[test]
    fn delta_presence_is_a_clear_error_not_a_fabricated_number() {
        let buckets = vec!["20-29".to_string()];
        let sensitive = vec!["flu".to_string()];
        let err = evaluate_risk_model(RiskModel::DeltaPresence, &buckets, &sensitive).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn evaluate_risk_model_dispatches_k_anonymity() {
        let buckets = vec!["20-29".to_string(), "20-29".to_string()];
        let sensitive = vec!["flu".to_string(), "flu".to_string()];
        assert_eq!(
            evaluate_risk_model(RiskModel::KAnonymity, &buckets, &sensitive).unwrap(),
            RiskAssessment::KAnonymity(2)
        );
    }

    /// The wasm-bindgen boundary (`apps/triplex-web`) round-trips this
    /// through JSON, so its shape is a contract worth pinning explicitly.
    #[test]
    fn risk_assessment_serializes_as_a_tagged_model_value_pair() {
        assert_eq!(
            serde_json::to_string(&RiskAssessment::KAnonymity(2)).unwrap(),
            r#"{"model":"k_anonymity","value":2}"#
        );
        assert_eq!(
            serde_json::to_string(&RiskAssessment::TCloseness(0.5)).unwrap(),
            r#"{"model":"t_closeness","value":0.5}"#
        );
    }

    /// Proves this crate's scoring has no dependency on `triplex-anonymizer`
    /// or any recipe — it can run on raw bucketed values from any source,
    /// which is what lets `apps/triplex-web` advise on a column's risk
    /// before the user has picked an anonymization method at all.
    #[test]
    fn runs_standalone_with_no_anonymizer_dependency() {
        let buckets: Vec<String> = vec!["0-9".into(), "0-9".into(), "10-19".into()];
        assert_eq!(k_anonymity(&buckets), 1);
    }
}
