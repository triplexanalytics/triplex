//! Group-fairness metrics across the groups of one protected attribute.
//!
//! Definitions (pinned to Fairlearn's between-groups definitions so an independent oracle can
//! check them; the card states them so a reader knows what was computed):
//!
//! - `demographic_parity_difference` = max - min selection rate across groups.
//! - `disparate_impact_ratio` = min / max selection rate (the four-fifths rule's quantity;
//!   equals Fairlearn's `demographic_parity_ratio`).
//! - `tpr_*` / `fpr_*` = the same max-min and min/max over per-group true-positive and
//!   false-positive rates; groups where a rate is undefined are left out of that rate.
//! - `equalized_odds_difference` = the larger of the TPR and FPR differences;
//!   `equalized_odds_ratio` = the smaller of the TPR and FPR ratios.
//!
//! A ratio is 1.0 when its maximum is 0 (every group has the same rate, zero), and `None` when
//! fewer than two groups have the rate defined.

use serde::Serialize;

/// One group's rates, as computed by `metrics::compute`.
#[derive(Debug, Clone)]
pub struct GroupRates {
    pub group: String,
    pub selection_rate: f64,
    pub tpr: Option<f64>,
    pub fpr: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fairness {
    pub demographic_parity_difference: Option<f64>,
    pub disparate_impact_ratio: Option<f64>,
    pub tpr_difference: Option<f64>,
    pub tpr_ratio: Option<f64>,
    pub fpr_difference: Option<f64>,
    pub fpr_ratio: Option<f64>,
    pub equalized_odds_difference: Option<f64>,
    pub equalized_odds_ratio: Option<f64>,
}

fn spread(values: &[f64]) -> Option<(f64, f64)> {
    if values.len() < 2 {
        return None;
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some((min, max))
}

fn difference(values: &[f64]) -> Option<f64> {
    spread(values).map(|(min, max)| max - min)
}

fn ratio(values: &[f64]) -> Option<f64> {
    spread(values).map(|(min, max)| if max == 0.0 { 1.0 } else { min / max })
}

fn max_of(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

fn min_of(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

pub fn compute(groups: &[GroupRates]) -> Fairness {
    let selection: Vec<f64> = groups.iter().map(|g| g.selection_rate).collect();
    let tpr: Vec<f64> = groups.iter().filter_map(|g| g.tpr).collect();
    let fpr: Vec<f64> = groups.iter().filter_map(|g| g.fpr).collect();
    let (tpr_difference, tpr_ratio) = (difference(&tpr), ratio(&tpr));
    let (fpr_difference, fpr_ratio) = (difference(&fpr), ratio(&fpr));
    Fairness {
        demographic_parity_difference: difference(&selection),
        disparate_impact_ratio: ratio(&selection),
        tpr_difference,
        tpr_ratio,
        fpr_difference,
        fpr_ratio,
        equalized_odds_difference: max_of(tpr_difference, fpr_difference),
        equalized_odds_ratio: min_of(tpr_ratio, fpr_ratio),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(name: &str, selection: f64, tpr: Option<f64>, fpr: Option<f64>) -> GroupRates {
        GroupRates {
            group: name.to_string(),
            selection_rate: selection,
            tpr,
            fpr,
        }
    }

    #[test]
    fn matches_the_hand_computed_values_for_two_groups() {
        // selection 0.6 vs 0.3 -> difference 0.3, ratio 0.5; tpr 0.9 vs 0.6 -> 0.3 / 2/3;
        // fpr 0.2 vs 0.1 -> 0.1 / 0.5; equalized odds: max diff 0.3, min ratio 0.5.
        let f = compute(&[
            g("a", 0.6, Some(0.9), Some(0.2)),
            g("b", 0.3, Some(0.6), Some(0.1)),
        ]);
        assert!((f.demographic_parity_difference.unwrap() - 0.3).abs() < 1e-12);
        assert!((f.disparate_impact_ratio.unwrap() - 0.5).abs() < 1e-12);
        assert!((f.tpr_ratio.unwrap() - 2.0 / 3.0).abs() < 1e-12);
        assert!((f.fpr_ratio.unwrap() - 0.5).abs() < 1e-12);
        assert!((f.equalized_odds_difference.unwrap() - 0.3).abs() < 1e-12);
        assert!((f.equalized_odds_ratio.unwrap() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_single_group_has_no_fairness_metrics() {
        let f = compute(&[g("a", 0.5, Some(0.5), Some(0.5))]);
        assert_eq!(f.demographic_parity_difference, None);
        assert_eq!(f.disparate_impact_ratio, None);
        assert_eq!(f.equalized_odds_ratio, None);
    }

    #[test]
    fn a_group_with_an_undefined_rate_is_left_out_of_that_rate_only() {
        let f = compute(&[
            g("a", 0.5, Some(0.8), Some(0.1)),
            g("b", 0.5, None, Some(0.2)),
            g("c", 0.5, Some(0.4), None),
        ]);
        assert!(
            (f.tpr_ratio.unwrap() - 0.5).abs() < 1e-12,
            "tpr compares a and c"
        );
        assert!(
            (f.fpr_ratio.unwrap() - 0.5).abs() < 1e-12,
            "fpr compares a and b"
        );
    }

    #[test]
    fn when_every_group_has_a_zero_rate_the_ratio_is_one_not_nan() {
        let f = compute(&[
            g("a", 0.0, Some(0.0), Some(0.0)),
            g("b", 0.0, Some(0.0), Some(0.0)),
        ]);
        assert_eq!(f.disparate_impact_ratio, Some(1.0));
        assert_eq!(f.equalized_odds_ratio, Some(1.0));
        assert_eq!(f.demographic_parity_difference, Some(0.0));
    }

    #[test]
    fn swapping_group_order_or_names_changes_nothing() {
        let forward = compute(&[
            g("a", 0.6, Some(0.9), Some(0.2)),
            g("b", 0.3, Some(0.6), Some(0.1)),
        ]);
        let reversed = compute(&[
            g("z", 0.3, Some(0.6), Some(0.1)),
            g("y", 0.6, Some(0.9), Some(0.2)),
        ]);
        assert_eq!(forward, reversed);
    }
}
