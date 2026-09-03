//! Numeric generalization/bucketing — the k-anonymity mechanism from
//! the design notes's Tier 1 row — plus the named risk
//! models a bucketed column can be checked against.

use std::collections::{HashMap, HashSet};

use triplex_core::{Error, Result};

use crate::mechanism::Mechanism;

pub(crate) struct Bucketer {
    width: u64,
}

impl Bucketer {
    pub(crate) fn new(width: u64) -> Self {
        Self {
            width: width.max(1),
        }
    }
}

impl Mechanism for Bucketer {
    fn mask(&self, value: &str) -> String {
        bucket_value(self.width, value)
    }
}

/// Replaces a numeric value with the `"lo-hi"` range it falls in, e.g.
/// width 10 turns `"23"` into `"20-29"`. A value that isn't a whole number
/// has nothing to generalize and passes through unchanged.
pub fn bucket_value(width: u64, value: &str) -> String {
    let Ok(n) = value.parse::<i64>() else {
        return value.to_string();
    };
    let width = width.max(1) as i64;
    let lo = n.div_euclid(width) * width;
    let hi = lo + width - 1;
    format!("{lo}-{hi}")
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskModel {
    KAnonymity,
    LDiversity,
    TCloseness,
    DeltaPresence,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
fn total_variation_distance(a: &HashMap<&str, f64>, b: &HashMap<&str, f64>) -> f64 {
    let categories: HashSet<&str> = a.keys().chain(b.keys()).copied().collect();
    let sum: f64 = categories
        .iter()
        .map(|c| (a.get(c).copied().unwrap_or(0.0) - b.get(c).copied().unwrap_or(0.0)).abs())
        .sum();
    sum / 2.0
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
    fn buckets_a_number_into_its_width_sized_range() {
        assert_eq!(bucket_value(10, "23"), "20-29");
        assert_eq!(bucket_value(10, "20"), "20-29");
        assert_eq!(bucket_value(10, "29"), "20-29");
        assert_eq!(bucket_value(10, "30"), "30-39");
    }

    #[test]
    fn negative_numbers_bucket_toward_negative_infinity_not_toward_zero() {
        // div_euclid, not integer truncation: -1 must land in "-10--1", not "0-9".
        assert_eq!(bucket_value(10, "-1"), "-10--1");
    }

    #[test]
    fn a_non_numeric_value_passes_through_unchanged() {
        assert_eq!(bucket_value(10, "n/a"), "n/a");
    }

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
}
