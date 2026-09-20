//! Fidelity and leakage metrics for a synthetic table against the real one it was fitted on.
//! These are measurements, not guarantees: a leakage number that looks fine does not certify
//! privacy (see `SynthesisReport::privacy_statement`).

use super::copula::ColumnData;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

/// Largest number of synthetic rows, and of real rows, used for the nearest-record search.
pub const DCR_SYNTHETIC_ROWS: usize = 1000;
pub const DCR_REAL_ROWS: usize = 5000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnFidelity {
    pub column: String,
    pub kind: &'static str,
    pub null_rate_real: f64,
    pub null_rate_synthetic: f64,
    /// Two-sample Kolmogorov-Smirnov statistic (0 identical, 1 disjoint); numeric columns.
    pub ks_statistic: Option<f64>,
    pub mean_real: Option<f64>,
    pub mean_synthetic: Option<f64>,
    pub std_real: Option<f64>,
    pub std_synthetic: Option<f64>,
    /// Total variation distance between category distributions; categorical columns.
    pub total_variation: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrelationFidelity {
    pub pairs_compared: usize,
    pub max_abs_error: Option<f64>,
    pub mean_abs_error: Option<f64>,
    pub note: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fidelity {
    pub columns: Vec<ColumnFidelity>,
    pub correlation: CorrelationFidelity,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Leakage {
    /// Synthetic rows identical (on every synthesized column) to some real row.
    pub exact_copy_rows: usize,
    pub exact_copy_rate: f64,
    pub nearest_record_synthetic_rows: usize,
    pub nearest_record_real_rows: usize,
    /// Distance to the closest real record (Gower-style, 0 identical, 1 maximally different).
    pub dcr_synthetic_min: Option<f64>,
    pub dcr_synthetic_p5: Option<f64>,
    pub dcr_synthetic_median: Option<f64>,
    /// The same statistic for real rows against the other real rows: the baseline a synthetic
    /// row should not be much closer than.
    pub dcr_real_to_real_median: Option<f64>,
    /// Synthetic median over real-to-real median. Well below 1 means the synthetic rows sit
    /// closer to real records than real records sit to each other.
    pub dcr_ratio: Option<f64>,
    pub note: &'static str,
}

fn mean_std(values: &[f64]) -> (Option<f64>, Option<f64>) {
    if values.is_empty() {
        return (None, None);
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    (Some(mean), Some(variance.sqrt()))
}

fn non_null_numbers(v: &[Option<f64>]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

pub fn ks_statistic(a: &[f64], b: &[f64]) -> Option<f64> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    a.sort_by(|x, y| x.total_cmp(y));
    b.sort_by(|x, y| x.total_cmp(y));
    let (mut i, mut j, mut worst) = (0usize, 0usize, 0.0f64);
    while i < a.len() && j < b.len() {
        let x = a[i].min(b[j]);
        while i < a.len() && a[i] <= x {
            i += 1;
        }
        while j < b.len() && b[j] <= x {
            j += 1;
        }
        worst = worst.max((i as f64 / a.len() as f64 - j as f64 / b.len() as f64).abs());
    }
    Some(worst)
}

fn distribution(values: &[Option<String>]) -> BTreeMap<&str, f64> {
    let mut counts: BTreeMap<&str, f64> = BTreeMap::new();
    let mut total = 0.0;
    for v in values.iter().flatten() {
        *counts.entry(v.as_str()).or_insert(0.0) += 1.0;
        total += 1.0;
    }
    for c in counts.values_mut() {
        *c /= total;
    }
    counts
}

fn total_variation(a: &[Option<String>], b: &[Option<String>]) -> Option<f64> {
    let (da, db) = (distribution(a), distribution(b));
    if da.is_empty() || db.is_empty() {
        return None;
    }
    let keys: HashSet<&str> = da.keys().chain(db.keys()).copied().collect();
    Some(
        keys.iter()
            .map(|k| (da.get(k).unwrap_or(&0.0) - db.get(k).unwrap_or(&0.0)).abs())
            .sum::<f64>()
            / 2.0,
    )
}

fn pearson(a: &[Option<f64>], b: &[Option<f64>]) -> Option<f64> {
    let pairs: Vec<(f64, f64)> = a
        .iter()
        .zip(b)
        .filter_map(|(x, y)| Some(((*x)?, (*y)?)))
        .collect();
    if pairs.len() < 3 {
        return None;
    }
    let n = pairs.len() as f64;
    let (ma, mb) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in &pairs {
        sab += (x - ma) * (y - mb);
        saa += (x - ma).powi(2);
        sbb += (y - mb).powi(2);
    }
    (saa > 1e-12 && sbb > 1e-12).then(|| sab / (saa.sqrt() * sbb.sqrt()))
}

/// `real_merged` is the real table after rare categories were merged (what the model was fitted
/// on); categorical fidelity compares against it.
pub fn fidelity(
    names: &[String],
    real_merged: &[ColumnData],
    synthetic: &[ColumnData],
) -> Fidelity {
    let mut columns = Vec::new();
    for ((name, r), s) in names.iter().zip(real_merged).zip(synthetic) {
        let rate = |nulls: usize, n: usize| nulls as f64 / n.max(1) as f64;
        match (r, s) {
            (ColumnData::Numeric(rv), ColumnData::Numeric(sv)) => {
                let (rn, sn) = (non_null_numbers(rv), non_null_numbers(sv));
                let ((mr, sr), (ms, ss)) = (mean_std(&rn), mean_std(&sn));
                columns.push(ColumnFidelity {
                    column: name.clone(),
                    kind: "numeric",
                    null_rate_real: rate(rv.len() - rn.len(), rv.len()),
                    null_rate_synthetic: rate(sv.len() - sn.len(), sv.len()),
                    ks_statistic: ks_statistic(&rn, &sn),
                    mean_real: mr,
                    mean_synthetic: ms,
                    std_real: sr,
                    std_synthetic: ss,
                    total_variation: None,
                });
            }
            (ColumnData::Categorical(rv), ColumnData::Categorical(sv)) => {
                let (rn, sn) = (rv.iter().flatten().count(), sv.iter().flatten().count());
                columns.push(ColumnFidelity {
                    column: name.clone(),
                    kind: "categorical",
                    null_rate_real: rate(rv.len() - rn, rv.len()),
                    null_rate_synthetic: rate(sv.len() - sn, sv.len()),
                    ks_statistic: None,
                    mean_real: None,
                    mean_synthetic: None,
                    std_real: None,
                    std_synthetic: None,
                    total_variation: total_variation(rv, sv),
                });
            }
            _ => unreachable!("real and synthetic columns share a kind"),
        }
    }

    let numeric: Vec<usize> = real_merged
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, ColumnData::Numeric(_)))
        .map(|(i, _)| i)
        .collect();
    let mut errors = Vec::new();
    for (pos, &i) in numeric.iter().enumerate() {
        for &j in &numeric[..pos] {
            let (
                ColumnData::Numeric(ri),
                ColumnData::Numeric(rj),
                ColumnData::Numeric(si),
                ColumnData::Numeric(sj),
            ) = (
                &real_merged[i],
                &real_merged[j],
                &synthetic[i],
                &synthetic[j],
            )
            else {
                continue;
            };
            if let (Some(real), Some(synth)) = (pearson(ri, rj), pearson(si, sj)) {
                errors.push((real - synth).abs());
            }
        }
    }
    Fidelity {
        columns,
        correlation: CorrelationFidelity {
            pairs_compared: errors.len(),
            max_abs_error: errors.iter().copied().fold(None, |m: Option<f64>, e| Some(m.map_or(e, |m| m.max(e)))),
            mean_abs_error: (!errors.is_empty()).then(|| errors.iter().sum::<f64>() / errors.len() as f64),
            note: "numeric column pairs only; correlations involving categorical columns are not measured",
        },
    }
}

/// A row as comparable keys: numeric bits or category strings, `None` for null.
fn row_key(columns: &[ColumnData], row: usize) -> String {
    columns
        .iter()
        .map(|c| match c {
            ColumnData::Numeric(v) => {
                v[row].map_or("\u{1}".to_string(), |x| format!("n{:x}", x.to_bits()))
            }
            ColumnData::Categorical(v) => v[row]
                .as_ref()
                .map_or("\u{1}".to_string(), |s| format!("c{s}")),
        })
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

/// Per-column ranges of the real numeric columns, used to scale distances.
fn ranges(real: &[ColumnData]) -> Vec<f64> {
    real.iter()
        .map(|c| match c {
            ColumnData::Numeric(v) => {
                let vals = non_null_numbers(v);
                let (min, max) = vals
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
                        (lo.min(x), hi.max(x))
                    });
                if vals.is_empty() {
                    0.0
                } else {
                    max - min
                }
            }
            ColumnData::Categorical(_) => 0.0,
        })
        .collect()
}

fn distance(a: &[ColumnData], ai: usize, b: &[ColumnData], bi: usize, range: &[f64]) -> f64 {
    let total: f64 = a
        .iter()
        .zip(b)
        .zip(range)
        .map(|((ca, cb), &r)| match (ca, cb) {
            (ColumnData::Numeric(x), ColumnData::Numeric(y)) => match (x[ai], y[bi]) {
                (Some(p), Some(q)) => {
                    if r > 0.0 {
                        ((p - q).abs() / r).min(1.0)
                    } else {
                        f64::from(p != q)
                    }
                }
                (None, None) => 0.0,
                _ => 1.0,
            },
            (ColumnData::Categorical(x), ColumnData::Categorical(y)) => f64::from(x[ai] != y[bi]),
            _ => 1.0,
        })
        .sum();
    total / a.len() as f64
}

fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let position = p * (sorted.len() - 1) as f64;
    let (lo, hi) = (position.floor() as usize, position.ceil() as usize);
    Some(sorted[lo] + (sorted[hi] - sorted[lo]) * (position - lo as f64))
}

/// `real` must be the ORIGINAL real values (categories not merged): a synthetic row equal to a
/// real record is what "copy" means.
pub fn leakage(real: &[ColumnData], synthetic: &[ColumnData]) -> Leakage {
    let real_rows = real.first().map_or(0, ColumnData::len);
    let synth_rows = synthetic.first().map_or(0, ColumnData::len);
    let real_keys: HashSet<String> = (0..real_rows).map(|i| row_key(real, i)).collect();
    let exact = (0..synth_rows)
        .filter(|&i| real_keys.contains(&row_key(synthetic, i)))
        .count();

    let range = ranges(real);
    let pool = real_rows.min(DCR_REAL_ROWS);
    let synth_sample = synth_rows.min(DCR_SYNTHETIC_ROWS);
    let real_sample = real_rows.min(DCR_SYNTHETIC_ROWS);
    let mut synthetic_dcr: Vec<f64> = (0..synth_sample)
        .map(|i| {
            (0..pool)
                .map(|j| distance(synthetic, i, real, j, &range))
                .fold(f64::INFINITY, f64::min)
        })
        .collect();
    let mut real_dcr: Vec<f64> = (0..real_sample)
        .map(|i| {
            (0..pool)
                .filter(|&j| j != i)
                .map(|j| distance(real, i, real, j, &range))
                .fold(f64::INFINITY, f64::min)
        })
        .filter(|d| d.is_finite())
        .collect();
    synthetic_dcr.sort_by(|a, b| a.total_cmp(b));
    real_dcr.sort_by(|a, b| a.total_cmp(b));
    let (synthetic_median, real_median) =
        (percentile(&synthetic_dcr, 0.5), percentile(&real_dcr, 0.5));
    Leakage {
        exact_copy_rows: exact,
        exact_copy_rate: exact as f64 / synth_rows.max(1) as f64,
        nearest_record_synthetic_rows: synth_sample,
        nearest_record_real_rows: pool,
        dcr_synthetic_min: synthetic_dcr.first().copied(),
        dcr_synthetic_p5: percentile(&synthetic_dcr, 0.05),
        dcr_synthetic_median: synthetic_median,
        dcr_real_to_real_median: real_median,
        dcr_ratio: match (synthetic_median, real_median) {
            (Some(s), Some(r)) if r > 0.0 => Some(s / r),
            _ => None,
        },
        note: "measured on the first rows of each table; a few columns of low cardinality produce exact copies by chance, so read the copy count together with the nearest-record ratio",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(v: &[f64]) -> ColumnData {
        ColumnData::Numeric(v.iter().map(|x| Some(*x)).collect())
    }
    fn cat(v: &[&str]) -> ColumnData {
        ColumnData::Categorical(v.iter().map(|x| Some(x.to_string())).collect())
    }

    #[test]
    fn mean_and_population_standard_deviation_match_hand_values() {
        assert_eq!(
            mean_std(&[1.0, 2.0, 3.0, 4.0]),
            (Some(2.5), Some(1.25_f64.sqrt()))
        );
        assert_eq!(mean_std(&[5.0]), (Some(5.0), Some(0.0)));
        assert_eq!(mean_std(&[]), (None, None));
        assert_eq!(mean_std(&[2.0, 4.0]), (Some(3.0), Some(1.0)));
    }

    #[test]
    fn fidelity_reports_means_and_standard_deviations() {
        let names = vec!["a".to_string()];
        let f = fidelity(&names, &[num(&[1.0, 2.0, 3.0])], &[num(&[2.0, 4.0, 6.0])]);
        let c = &f.columns[0];
        assert_eq!((c.mean_real, c.mean_synthetic), (Some(2.0), Some(4.0)));
        assert!((c.std_real.unwrap() - (2.0_f64 / 3.0).sqrt()).abs() < 1e-12);
        assert!((c.std_synthetic.unwrap() - (8.0_f64 / 3.0).sqrt()).abs() < 1e-12);
    }

    #[test]
    fn pearson_has_the_right_sign_size_and_degenerate_cases() {
        let some = |v: &[f64]| v.iter().map(|x| Some(*x)).collect::<Vec<_>>();
        let x = some(&[1.0, 2.0, 3.0, 4.0]);
        assert!((pearson(&x, &some(&[10.0, 20.0, 30.0, 40.0])).unwrap() - 1.0).abs() < 1e-12);
        assert!((pearson(&x, &some(&[4.0, 3.0, 2.0, 1.0])).unwrap() + 1.0).abs() < 1e-12);
        assert!((pearson(&x, &some(&[2.0, 1.0, 4.0, 3.0])).unwrap() - 0.6).abs() < 1e-12);
        assert_eq!(
            pearson(&x, &some(&[7.0; 4])),
            None,
            "a constant column is undefined"
        );
        assert_eq!(pearson(&some(&[7.0; 4]), &x), None);
        assert_eq!(
            pearson(&some(&[1.0, 2.0]), &some(&[1.0, 2.0])),
            None,
            "two pairs are too few"
        );
        assert!(
            pearson(&some(&[1.0, 2.0, 3.0]), &some(&[1.0, 2.0, 3.0])).is_some(),
            "three are enough"
        );
    }

    #[test]
    fn total_variation_is_undefined_when_either_side_has_no_values() {
        let some: Vec<Option<String>> = vec![Some("a".into())];
        let none: Vec<Option<String>> = vec![None];
        assert_eq!(total_variation(&some, &none), None);
        assert_eq!(total_variation(&none, &some), None);
        assert_eq!(total_variation(&some, &some), Some(0.0));
    }

    #[test]
    fn percentiles_interpolate_between_order_statistics() {
        let v = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&v, 0.0), Some(1.0));
        assert_eq!(percentile(&v, 1.0), Some(4.0));
        assert_eq!(percentile(&v, 0.5), Some(2.5));
        assert!((percentile(&v, 0.05).unwrap() - 1.15).abs() < 1e-12);
        assert_eq!(percentile(&[7.0], 0.5), Some(7.0));
        assert_eq!(percentile(&[], 0.5), None);
    }

    #[test]
    fn numeric_distance_is_scaled_by_the_real_range() {
        // values 5, 9, 15: the range is 10 (not 20), so 5 vs 9 is 0.4.
        let a = [num(&[5.0, 9.0, 15.0])];
        let range = ranges(&a);
        assert_eq!(range, vec![10.0]);
        assert!((distance(&a, 0, &a, 1, &range) - 0.4).abs() < 1e-12);
        assert_eq!(distance(&a, 0, &a, 2, &range), 1.0, "capped at 1");
    }

    #[test]
    fn distance_handles_constant_columns_and_double_nulls() {
        let constant = [num(&[3.0, 3.0])];
        let r = ranges(&constant);
        assert_eq!(
            distance(&constant, 0, &constant, 1, &r),
            0.0,
            "equal values in a zero-range column"
        );
        let other = [num(&[3.0, 4.0])];
        assert_eq!(
            distance(&constant, 0, &other, 1, &ranges(&constant)),
            1.0,
            "different values in a zero-range column"
        );
        let nulls = [ColumnData::Numeric(vec![None, None, Some(1.0)])];
        let range = ranges(&nulls);
        assert_eq!(
            distance(&nulls, 0, &nulls, 1, &range),
            0.0,
            "both null is a match"
        );
        assert_eq!(
            distance(&nulls, 0, &nulls, 2, &range),
            1.0,
            "one null is a mismatch"
        );
    }

    #[test]
    fn the_nearest_record_ratio_is_undefined_when_real_rows_already_coincide() {
        // identical real rows -> real-to-real distance 0 -> a ratio would divide by zero.
        let real = [num(&[1.0, 1.0, 1.0, 1.0]), cat(&["a", "a", "a", "a"])];
        assert_eq!(leakage(&real, &real).dcr_ratio, None);
        let spread = [num(&[1.0, 4.0, 9.0, 16.0]), cat(&["a", "b", "a", "b"])];
        let synthetic = [num(&[2.0, 5.0]), cat(&["a", "b"])];
        assert!(leakage(&spread, &synthetic).dcr_ratio.is_some());
    }

    #[test]
    fn ks_is_zero_for_identical_samples_and_one_for_disjoint_ones() {
        assert_eq!(ks_statistic(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]), Some(0.0));
        assert_eq!(ks_statistic(&[1.0, 2.0], &[10.0, 20.0]), Some(1.0));
        assert_eq!(ks_statistic(&[], &[1.0]), None);
    }

    #[test]
    fn ks_matches_a_hand_computed_value() {
        // a = {1,2,3,4}, b = {2.5,3.5,4.5,5.5}: the largest CDF gap is at x=2 (0.5 vs 0) -> 0.5.
        assert_eq!(
            ks_statistic(&[1.0, 2.0, 3.0, 4.0], &[2.5, 3.5, 4.5, 5.5]),
            Some(0.5)
        );
    }

    #[test]
    fn total_variation_is_half_the_l1_distance() {
        let a: Vec<Option<String>> = ["x", "x", "y", "y"]
            .iter()
            .map(|s| Some(s.to_string()))
            .collect();
        let b: Vec<Option<String>> = ["x", "x", "x", "y"]
            .iter()
            .map(|s| Some(s.to_string()))
            .collect();
        assert_eq!(total_variation(&a, &b), Some(0.25));
        assert_eq!(total_variation(&a, &a), Some(0.0));
    }

    #[test]
    fn correlation_error_compares_numeric_pairs() {
        let names = vec!["a".to_string(), "b".to_string()];
        let real = [
            num(&[1.0, 2.0, 3.0, 4.0, 5.0]),
            num(&[2.0, 4.0, 6.0, 8.0, 10.0]),
        ];
        let flipped = [
            num(&[1.0, 2.0, 3.0, 4.0, 5.0]),
            num(&[10.0, 8.0, 6.0, 4.0, 2.0]),
        ];
        let f = fidelity(&names, &real, &flipped);
        assert_eq!(f.correlation.pairs_compared, 1);
        assert!(
            (f.correlation.max_abs_error.unwrap() - 2.0).abs() < 1e-12,
            "1 vs -1 differ by 2"
        );
        let same = fidelity(&names, &real, &real);
        assert_eq!(same.correlation.max_abs_error, Some(0.0));
    }

    #[test]
    fn an_exact_copy_is_counted_and_a_novel_row_is_not() {
        let real = [num(&[1.0, 2.0, 3.0]), cat(&["a", "b", "c"])];
        let synthetic = [num(&[2.0, 9.0]), cat(&["b", "b"])];
        let l = leakage(&real, &synthetic);
        assert_eq!(l.exact_copy_rows, 1);
        assert_eq!(l.exact_copy_rate, 0.5);
    }

    #[test]
    fn a_verbatim_copy_of_the_real_table_has_zero_nearest_record_distance() {
        let real = [num(&[1.0, 5.0, 9.0, 13.0]), cat(&["a", "b", "a", "b"])];
        let l = leakage(&real, &real);
        assert_eq!(l.exact_copy_rows, 4);
        assert_eq!(l.dcr_synthetic_min, Some(0.0));
        assert_eq!(l.dcr_synthetic_median, Some(0.0));
    }

    #[test]
    fn distance_treats_null_mismatch_as_maximal_and_scales_numbers_by_range() {
        let a = [
            ColumnData::Numeric(vec![Some(0.0), None]),
            num(&[0.0, 10.0]),
        ];
        let range = ranges(&a);
        assert_eq!(distance(&a, 0, &a, 0, &range), 0.0);
        // column 0: one null vs a value -> 1; column 1: |0-10|/10 = 1; mean 1.
        assert_eq!(distance(&a, 0, &a, 1, &range), 1.0);
    }
}
