//! Binary-classification metrics for one slice. Every ratio whose denominator is zero is
//! `None` (undefined), never 0: a group with no actual positives has no true-positive rate, and
//! reporting 0 would read as "the model missed everyone".

use serde::Serialize;

/// One evaluation row after label normalization: true label, predicted label, optional score.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub actual: bool,
    pub predicted: bool,
    pub score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Metrics {
    /// Share of the slice the model predicted positive (a.k.a. selection rate).
    pub selection_rate: f64,
    /// Share of the slice that is actually positive.
    pub base_rate: f64,
    pub accuracy: f64,
    pub precision: Option<f64>,
    /// True-positive rate.
    pub recall: Option<f64>,
    /// False-positive rate.
    pub fpr: Option<f64>,
    pub f1: Option<f64>,
    pub roc_auc: Option<f64>,
}

fn ratio(num: usize, den: usize) -> Option<f64> {
    (den > 0).then(|| num as f64 / den as f64)
}

/// Rank-based (Mann-Whitney) ROC-AUC with tied scores given their average rank, the same
/// definition scikit-learn's `roc_auc_score` uses. `None` unless the slice has both classes.
pub fn roc_auc(rows: &[Row]) -> Option<f64> {
    let mut scored: Vec<(f64, bool)> = rows
        .iter()
        .filter_map(|r| r.score.map(|s| (s, r.actual)))
        .collect();
    let positives = scored.iter().filter(|(_, y)| *y).count();
    let negatives = scored.len() - positives;
    if positives == 0 || negatives == 0 {
        return None;
    }
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut positive_rank_sum = 0.0;
    let mut i = 0;
    while i < scored.len() {
        let mut j = i;
        while j + 1 < scored.len() && scored[j + 1].0 == scored[i].0 {
            j += 1;
        }
        // 1-based ranks i+1..=j+1 share their average.
        let average_rank = (i + 1 + j + 1) as f64 / 2.0;
        positive_rank_sum += average_rank * scored[i..=j].iter().filter(|(_, y)| *y).count() as f64;
        i = j + 1;
    }
    let p = positives as f64;
    Some((positive_rank_sum - p * (p + 1.0) / 2.0) / (p * negatives as f64))
}

/// Metrics over `rows`, which must be non-empty.
pub fn compute(rows: &[Row]) -> Metrics {
    let (mut tp, mut fp, mut tn, mut fn_) = (0usize, 0usize, 0usize, 0usize);
    for r in rows {
        match (r.actual, r.predicted) {
            (true, true) => tp += 1,
            (false, true) => fp += 1,
            (false, false) => tn += 1,
            (true, false) => fn_ += 1,
        }
    }
    let n = rows.len();
    Metrics {
        selection_rate: (tp + fp) as f64 / n as f64,
        base_rate: (tp + fn_) as f64 / n as f64,
        accuracy: (tp + tn) as f64 / n as f64,
        precision: ratio(tp, tp + fp),
        recall: ratio(tp, tp + fn_),
        fpr: ratio(fp, fp + tn),
        f1: ratio(2 * tp, 2 * tp + fp + fn_),
        roc_auc: roc_auc(rows),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(actual: bool, predicted: bool, score: Option<f64>) -> Row {
        Row {
            actual,
            predicted,
            score,
        }
    }

    /// tp=3 fp=1 tn=4 fn=2 over 10 rows, worked out by hand:
    /// accuracy 7/10, precision 3/4, recall 3/5, fpr 1/5, f1 6/9, selection 4/10, base 5/10.
    fn hand_case() -> Vec<Row> {
        let mut rows = Vec::new();
        rows.extend((0..3).map(|_| row(true, true, None)));
        rows.extend((0..1).map(|_| row(false, true, None)));
        rows.extend((0..4).map(|_| row(false, false, None)));
        rows.extend((0..2).map(|_| row(true, false, None)));
        rows
    }

    #[test]
    fn confusion_metrics_match_the_hand_computed_values() {
        let m = compute(&hand_case());
        assert_eq!(m.accuracy, 0.7);
        assert_eq!(m.precision, Some(0.75));
        assert_eq!(m.recall, Some(0.6));
        assert_eq!(m.fpr, Some(0.2));
        assert_eq!(m.f1, Some(6.0 / 9.0));
        assert_eq!(m.selection_rate, 0.4);
        assert_eq!(m.base_rate, 0.5);
        assert_eq!(m.roc_auc, None, "no scores were supplied");
    }

    #[test]
    fn a_slice_with_no_actual_positives_has_undefined_recall_not_zero() {
        let m = compute(&[row(false, false, None), row(false, true, None)]);
        assert_eq!(m.recall, None);
        assert_eq!(m.fpr, Some(0.5));
    }

    #[test]
    fn a_slice_with_no_actual_negatives_has_undefined_fpr() {
        let m = compute(&[row(true, true, None), row(true, false, None)]);
        assert_eq!(m.fpr, None);
        assert_eq!(m.recall, Some(0.5));
    }

    #[test]
    fn nothing_predicted_positive_makes_precision_undefined() {
        let m = compute(&[row(true, false, None), row(false, false, None)]);
        assert_eq!(m.precision, None);
        assert_eq!(m.f1, Some(0.0), "2tp / (2tp + fp + fn) is 0/1, defined");
    }

    #[test]
    fn a_perfect_ranking_has_auc_one_and_a_reversed_one_has_zero() {
        let good = [
            row(false, false, Some(0.1)),
            row(false, false, Some(0.2)),
            row(true, true, Some(0.8)),
            row(true, true, Some(0.9)),
        ];
        let bad = [
            row(true, false, Some(0.1)),
            row(true, false, Some(0.2)),
            row(false, true, Some(0.8)),
            row(false, true, Some(0.9)),
        ];
        assert_eq!(roc_auc(&good), Some(1.0));
        assert_eq!(roc_auc(&bad), Some(0.0));
    }

    #[test]
    fn tied_scores_share_their_average_rank() {
        // one positive and one negative with the same score: AUC is exactly 0.5.
        let tied = [row(true, true, Some(0.5)), row(false, true, Some(0.5))];
        assert_eq!(roc_auc(&tied), Some(0.5));
        // two negatives 0.2/0.5, two positives 0.5/0.9: pairs (pos>neg): 0.5>0.2, 0.9>0.2,
        // 0.5=0.5 counts half, 0.9>0.5 -> (1 + 1 + 0.5 + 1) / 4 = 0.875.
        let mixed = [
            row(false, false, Some(0.2)),
            row(false, false, Some(0.5)),
            row(true, true, Some(0.5)),
            row(true, true, Some(0.9)),
        ];
        assert_eq!(roc_auc(&mixed), Some(0.875));
    }

    #[test]
    fn auc_is_undefined_with_a_single_class() {
        assert_eq!(
            roc_auc(&[row(true, true, Some(0.9)), row(true, true, Some(0.8))]),
            None
        );
    }

    #[test]
    fn metrics_do_not_depend_on_row_order() {
        let mut rows = hand_case();
        let forward = compute(&rows);
        rows.reverse();
        assert_eq!(compute(&rows), forward);
    }
}
