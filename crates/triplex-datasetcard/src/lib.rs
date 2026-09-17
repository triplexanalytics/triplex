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

mod ebm;
mod suda;
pub use ebm::ebm_scores;
pub use suda::{dis_suda_scores, suda_scores, MAX_QUASI_IDENTIFIER_COLUMNS};

/// Which technique computes a row's disclosure-risk score — see
/// `suda.rs`/`ebm.rs`'s module docs for how each one works.
/// `row_risk_scores` dispatches on this the same way `evaluate_risk_model`
/// dispatches on `RiskModel`, so switching between them (a product decision
/// per the design notes: "implement both, switchable
/// on request" rather than pick one) is a one-line call-site change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowRiskModel {
    Suda,
    Ebm,
}

/// Every row's disclosure-risk score under `model`. `columns[j][i]` is
/// quasi-identifier column `j`'s value for row `i`. `Suda` additionally
/// enforces `MAX_QUASI_IDENTIFIER_COLUMNS` (see `suda_scores`); `Ebm` has no
/// such cap.
pub fn row_risk_scores(model: RowRiskModel, columns: &[Vec<String>]) -> Result<Vec<f64>> {
    match model {
        RowRiskModel::Suda => suda_scores(columns),
        RowRiskModel::Ebm => ebm_scores(columns),
    }
}

/// A named formal-privacy model a bucketed (generalized) column can be
/// checked against — see the design notes's Tier 1 row.
/// `evaluate_risk_model` computes every variant except δ-presence and k-map
/// directly from values already in the dataset. Those two are different in
/// kind: both bound risk relative to a *population* the dataset is only a
/// sample of (δ-presence: the probability an individual is present in the
/// dataset at all; k-map: an equivalence class's true population size, not
/// its sample size), which needs a reference population table strictly
/// larger than this dataset — a concept nothing in this crate has yet.
/// ponytail: δ-presence and k-map named but unimplemented; add a
/// population-table type and real arms here once one exists, rather than
/// fake a result now — see the project guidelines's rule that a privacy claim is never
/// asserted without a passing test backing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskModel {
    KAnonymity,
    LDiversity,
    TCloseness,
    DeltaPresence,
    /// Cao & Karras 2012's "basic β-likeness": bounds how much an
    /// equivalence class's local distribution of the sensitive attribute
    /// can *raise* an attacker's belief in any one value relative to the
    /// dataset-wide distribution — see `beta_likeness`'s doc comment for
    /// the exact formula and how it differs from `TCloseness`.
    BetaLikeness,
    /// Brickell & Shmatikov 2008's δ-disclosure privacy: like
    /// `BetaLikeness` but symmetric — it also bounds a class *lowering*
    /// belief in a value (e.g. "this group definitely doesn't have
    /// diagnosis X"), not just raising it. See `delta_disclosure`.
    DeltaDisclosure,
    KMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "model", content = "value", rename_all = "snake_case")]
pub enum RiskAssessment {
    KAnonymity(usize),
    LDiversity(usize),
    TCloseness(f64),
    BetaLikeness(f64),
    DeltaDisclosure(f64),
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

/// The bucket value achieving `k_anonymity`'s minimum — the smallest
/// equivalence class's own key, not just its size, so a caller can filter
/// rows down to exactly the group the score is about. Works identically for
/// a single column's own buckets or a `compose_quasi_identifiers` composite
/// (the caller decides which by what it passes in, same as `k_anonymity`
/// itself). `None` only for an empty `bucketed_values`. Ties broken by
/// `HashMap` iteration order — deterministic within one run, not a
/// documented tie-break rule, since which of several equally-smallest
/// classes gets shown first isn't a meaningful distinction here.
pub fn k_anonymity_worst_group(bucketed_values: &[String]) -> Option<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for v in bucketed_values {
        *counts.entry(v.as_str()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .min_by_key(|(_, count)| *count)
        .map(|(key, _)| key.to_string())
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

/// The bucket value achieving `l_diversity`'s minimum, same "which group"
/// counterpart `k_anonymity_worst_group` is to `k_anonymity`. `None` only
/// for empty input.
pub fn l_diversity_worst_group(
    bucketed_values: &[String],
    sensitive_values: &[String],
) -> Option<String> {
    let mut classes: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (bucket, sensitive) in bucketed_values.iter().zip(sensitive_values) {
        classes
            .entry(bucket.as_str())
            .or_default()
            .insert(sensitive.as_str());
    }
    classes
        .into_iter()
        .min_by_key(|(_, set)| set.len())
        .map(|(key, _)| key.to_string())
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

/// The bucket value achieving `t_closeness`'s maximum -- the equivalence
/// class whose sensitive-attribute mix strays furthest from the
/// dataset-wide distribution. `None` only for empty input. Ties broken by
/// `HashMap` iteration order, same as `k_anonymity_worst_group`.
pub fn t_closeness_worst_group(
    bucketed_values: &[String],
    sensitive_values: &[String],
) -> Option<String> {
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
        .into_iter()
        .map(|(key, class_values)| {
            let distance = total_variation_distance(&overall, &distribution(&class_values));
            (key, distance)
        })
        .fold(
            None,
            |best: Option<(&str, f64)>, (key, distance)| match best {
                Some((_, best_distance)) if best_distance >= distance => best,
                _ => Some((key, distance)),
            },
        )
        .map(|(key, _)| key.to_string())
}

/// Basic β-likeness (Cao & Karras 2012): the largest "belief raise" an
/// attacker gets from any one equivalence class about any one sensitive
/// value — `ln(local_share / global_share)`, but only counted where a
/// class's local share of a value is *higher* than that value's share
/// across the whole dataset (a class that under-represents a value isn't
/// counted here — see `delta_disclosure` for the symmetric version). 0.0
/// (tightest) means no class ever raises belief in any value above its
/// baseline rate; a larger number means some class makes some value look
/// more likely than the dataset as a whole suggests. Complements
/// `t_closeness`: t-closeness reports one aggregate distance per class
/// (how different the *whole mix* looks), β-likeness reports the worst
/// single value's belief shift — two classes with the same t-closeness
/// distance can have very different β-likeness if one spreads its
/// deviation across many values and the other concentrates it in one.
pub fn beta_likeness(bucketed_values: &[String], sensitive_values: &[String]) -> f64 {
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
        .map(|class_values| {
            let local = distribution(class_values);
            local
                .iter()
                .filter_map(|(value, local_share)| {
                    let global_share = overall.get(value).copied().unwrap_or(0.0);
                    (*local_share >= global_share && global_share > 0.0)
                        .then(|| (local_share / global_share).ln())
                })
                .fold(0.0, f64::max)
        })
        .fold(0.0, f64::max)
}

/// The bucket value achieving `beta_likeness`'s maximum -- the equivalence
/// class that raises belief in some sensitive value the most above its
/// dataset-wide baseline. `None` only for empty input. Ties broken by
/// `HashMap` iteration order, same as `k_anonymity_worst_group`.
pub fn beta_likeness_worst_group(
    bucketed_values: &[String],
    sensitive_values: &[String],
) -> Option<String> {
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
        .into_iter()
        .map(|(key, class_values)| {
            let local = distribution(&class_values);
            let score = local
                .iter()
                .filter_map(|(value, local_share)| {
                    let global_share = overall.get(value).copied().unwrap_or(0.0);
                    (*local_share >= global_share && global_share > 0.0)
                        .then(|| (local_share / global_share).ln())
                })
                .fold(0.0, f64::max);
            (key, score)
        })
        .fold(None, |best: Option<(&str, f64)>, (key, score)| match best {
            Some((_, best_score)) if best_score >= score => best,
            _ => Some((key, score)),
        })
        .map(|(key, _)| key.to_string())
}

/// δ-disclosure privacy (Brickell & Shmatikov 2008): the largest belief
/// *shift* an attacker gets from any one equivalence class about any one
/// sensitive value, in either direction — `|ln(local_share / global_share)|`
/// — unlike `beta_likeness`, a class that makes a value look *less* likely
/// than the dataset baseline (e.g. "this group definitely isn't the rare
/// diagnosis") counts too, since ruling something out is also a disclosure.
/// 0.0 (tightest) means every class's local distribution matches the
/// dataset-wide one exactly for every value it contains.
pub fn delta_disclosure(bucketed_values: &[String], sensitive_values: &[String]) -> f64 {
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
        .map(|class_values| {
            let local = distribution(class_values);
            local
                .iter()
                .map(|(value, local_share)| {
                    let global_share = overall.get(value).copied().unwrap_or(0.0);
                    if global_share > 0.0 {
                        (local_share / global_share).ln().abs()
                    } else {
                        0.0
                    }
                })
                .fold(0.0, f64::max)
        })
        .fold(0.0, f64::max)
}

/// The bucket value achieving `delta_disclosure`'s maximum -- the
/// equivalence class whose belief shift (either direction) about some
/// sensitive value is largest. `None` only for empty input. Ties broken by
/// `HashMap` iteration order, same as `k_anonymity_worst_group`.
pub fn delta_disclosure_worst_group(
    bucketed_values: &[String],
    sensitive_values: &[String],
) -> Option<String> {
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
        .into_iter()
        .map(|(key, class_values)| {
            let local = distribution(&class_values);
            let score = local
                .iter()
                .map(|(value, local_share)| {
                    let global_share = overall.get(value).copied().unwrap_or(0.0);
                    if global_share > 0.0 {
                        (local_share / global_share).ln().abs()
                    } else {
                        0.0
                    }
                })
                .fold(0.0, f64::max);
            (key, score)
        })
        .fold(None, |best: Option<(&str, f64)>, (key, score)| match best {
            Some((_, best_score)) if best_score >= score => best,
            _ => Some((key, score)),
        })
        .map(|(key, _)| key.to_string())
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
        RiskModel::BetaLikeness => Ok(RiskAssessment::BetaLikeness(beta_likeness(
            bucketed_values,
            sensitive_values,
        ))),
        RiskModel::DeltaDisclosure => Ok(RiskAssessment::DeltaDisclosure(delta_disclosure(
            bucketed_values,
            sensitive_values,
        ))),
        RiskModel::KMap => Err(Error::Validation(
            "k-map needs a reference population table, which isn't implemented yet".to_string(),
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
    fn k_anonymity_worst_group_names_the_smallest_class() {
        let buckets = vec!["20-29", "20-29", "20-29", "30-39", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(k_anonymity_worst_group(&buckets), Some("30-39".to_string()));
    }

    #[test]
    fn k_anonymity_worst_group_is_none_for_no_rows() {
        assert_eq!(k_anonymity_worst_group(&[]), None);
    }

    #[test]
    fn k_anonymity_worst_group_matches_k_anonymitys_own_size() {
        let buckets = vec!["a", "a", "a", "b", "b", "c"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let worst = k_anonymity_worst_group(&buckets).unwrap();
        let worst_size = buckets.iter().filter(|v| **v == worst).count();
        assert_eq!(worst_size, k_anonymity(&buckets));
        assert_eq!(worst, "c"); // the only singleton class
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
    fn l_diversity_worst_group_names_the_least_diverse_class() {
        let buckets = vec!["20-29", "20-29", "20-29", "30-39", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = vec!["flu", "flu", "flu", "flu", "diabetes"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(
            l_diversity_worst_group(&buckets, &diagnoses),
            Some("20-29".to_string())
        );
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
    fn t_closeness_worst_group_names_the_most_divergent_class() {
        let buckets = vec!["20-29", "20-29", "20-29", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = vec!["flu", "flu", "flu", "diabetes"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(
            t_closeness_worst_group(&buckets, &diagnoses),
            Some("30-39".to_string())
        );
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
    fn k_map_is_a_clear_error_not_a_fabricated_number() {
        let buckets = vec!["20-29".to_string()];
        let sensitive = vec!["flu".to_string()];
        let err = evaluate_risk_model(RiskModel::KMap, &buckets, &sensitive).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn beta_likeness_is_zero_when_every_class_matches_the_global_distribution() {
        let buckets = vec!["20-29", "20-29", "30-39", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = vec!["flu", "diabetes", "flu", "diabetes"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(beta_likeness(&buckets, &diagnoses), 0.0);
    }

    #[test]
    fn beta_likeness_is_positive_when_a_class_raises_belief_in_a_value() {
        // Global: flu 3/4, diabetes 1/4 (3 flu + 1 diabetes across both
        // classes). The singleton "30-39" class is 100% diabetes, raising
        // belief in "diabetes" from 0.25 to 1.0 — ln(1.0/0.25) = ln(4), the
        // largest ratio in this dataset (bigger than "20-29"'s flu ratio,
        // ln(1.0/0.75) ≈ 0.29, since raising a *rare* global value all the
        // way to 100% swings its ratio further than raising a common one).
        let buckets = vec!["20-29", "20-29", "20-29", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = vec!["flu", "flu", "flu", "diabetes"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let beta = beta_likeness(&buckets, &diagnoses);
        assert!(beta > 0.0);
        assert_eq!(beta, 4.0_f64.ln());
    }

    #[test]
    fn beta_likeness_worst_group_names_the_class_with_the_biggest_belief_raise() {
        let buckets = vec!["20-29", "20-29", "20-29", "30-39"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = vec!["flu", "flu", "flu", "diabetes"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(
            beta_likeness_worst_group(&buckets, &diagnoses),
            Some("30-39".to_string())
        );
    }

    /// Hand-verified two-class, three-value dataset where the single
    /// biggest belief *shift* in either direction is a *lowered* value —
    /// proving `delta_disclosure` (which considers both directions) and
    /// `beta_likeness` (which only ever considers a raised value) can
    /// genuinely disagree, not just on a technicality. Global: G 50%
    /// (10/20), R1 30% (6/20), R2 20% (4/20).
    /// - Class "A" (10 rows: 1 G, 5 R1, 4 R2): G's local share drops to 10%
    ///   (lowered: `|ln(0.1/0.5)| = ln(5)`, the biggest swing anywhere in
    ///   this dataset); R1 rises to 50% (`ln(0.5/0.3) ≈ 0.51`); R2 rises to
    ///   40% (`ln(0.4/0.2) = ln(2)`, the biggest *raised* swing).
    /// - Class "B" (10 rows: 9 G, 1 R1, 0 R2): G rises to 90%
    ///   (`ln(0.9/0.5) ≈ 0.59`); R1 drops to 10% (`|ln(0.1/0.3)| ≈ 1.10`);
    ///   R2 absent, never visited (see `delta_disclosure`'s doc comment).
    ///
    /// So `beta_likeness` (raised-only) maxes out at R2's `ln(2)`, while
    /// `delta_disclosure` (both directions) maxes out at G's lowered
    /// `ln(5)` — a real, different answer, not just a bigger one by chance.
    #[test]
    fn delta_disclosure_can_diverge_from_beta_likeness_on_a_lowered_value() {
        let buckets = ["A"; 10]
            .into_iter()
            .chain(["B"; 10])
            .map(String::from)
            .collect::<Vec<_>>();
        let diagnoses = ["G", "R1", "R1", "R1", "R1", "R1", "R2", "R2", "R2", "R2"]
            .into_iter()
            .chain(["G", "G", "G", "G", "G", "G", "G", "G", "G", "R1"])
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(buckets.len(), 20);
        assert_eq!(diagnoses.len(), 20);

        let beta = beta_likeness(&buckets, &diagnoses);
        let delta = delta_disclosure(&buckets, &diagnoses);
        assert_eq!(beta, 2.0_f64.ln());
        assert_eq!(delta, 5.0_f64.ln());
        assert!(delta > beta, "delta ({delta}) should exceed beta ({beta})");

        // Class "A"'s lowered G share (ln(5)) is the dataset's biggest
        // shift in either direction — bigger than class "B"'s own largest
        // shift (R1 dropping, |ln(0.1/0.3)| ≈ 1.10 < ln(5) ≈ 1.61).
        assert_eq!(
            delta_disclosure_worst_group(&buckets, &diagnoses),
            Some("A".to_string())
        );
    }

    #[test]
    fn row_risk_scores_dispatches_suda_and_ebm() {
        let a = vec!["x".to_string(), "x".to_string(), "y".to_string()];
        let b = vec!["p".to_string(), "p".to_string(), "q".to_string()];
        let columns = [a, b];
        let suda = row_risk_scores(RowRiskModel::Suda, &columns).unwrap();
        let ebm = row_risk_scores(RowRiskModel::Ebm, &columns).unwrap();
        assert_eq!(suda, suda_scores(&columns).unwrap());
        // Row 2 is the only (A=y, B=q) row — both techniques should agree
        // it's the riskiest of the three, even though their scales differ.
        assert!(suda[2] > suda[0]);
        assert!(ebm[2] > ebm[0]);
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
        assert_eq!(
            serde_json::to_string(&RiskAssessment::BetaLikeness(0.4)).unwrap(),
            r#"{"model":"beta_likeness","value":0.4}"#
        );
        assert_eq!(
            serde_json::to_string(&RiskAssessment::DeltaDisclosure(0.7)).unwrap(),
            r#"{"model":"delta_disclosure","value":0.7}"#
        );
    }

    #[test]
    fn evaluate_risk_model_dispatches_beta_likeness_and_delta_disclosure() {
        let buckets = vec!["20-29".to_string(), "20-29".to_string()];
        let sensitive = vec!["flu".to_string(), "flu".to_string()];
        assert_eq!(
            evaluate_risk_model(RiskModel::BetaLikeness, &buckets, &sensitive).unwrap(),
            RiskAssessment::BetaLikeness(0.0)
        );
        assert_eq!(
            evaluate_risk_model(RiskModel::DeltaDisclosure, &buckets, &sensitive).unwrap(),
            RiskAssessment::DeltaDisclosure(0.0)
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
