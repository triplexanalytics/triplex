//! Gaussian copula: each column keeps its own empirical marginal, and the dependence between
//! columns is one correlation matrix of their normal scores. Sampling draws correlated
//! standard normals, turns them into uniforms with the normal CDF, and maps each uniform back
//! through its column's marginal.
//!
//! Approximations, all reported by the fidelity report rather than hidden:
//! - a categorical column enters through the mid-point of its cumulative-frequency interval,
//!   which attenuates correlations involving categoricals;
//! - nulls are modelled per column and independently of the other columns;
//! - the correlation matrix is estimated on pairwise-complete rows and, if it is not positive
//!   definite, shrunk toward the identity until it is.

use super::numeric::{cholesky, phi, phi_inv, Rng};

/// One column's values. `None` is a null.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnData {
    Numeric(Vec<Option<f64>>),
    Categorical(Vec<Option<String>>),
}

impl ColumnData {
    pub fn len(&self) -> usize {
        match self {
            ColumnData::Numeric(v) => v.len(),
            ColumnData::Categorical(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// An integer column with at most this many distinct values is discrete (a code, a rating, a
/// count): it is sampled from its empirical distribution without interpolation, so it can only
/// produce values that occur in the data.
const DISCRETE_MAX_DISTINCT: usize = 20;

#[derive(Debug, Clone)]
enum Marginal {
    Numeric {
        sorted: Vec<f64>,
        integer: bool,
        discrete: bool,
        null_rate: f64,
    },
    Categorical {
        categories: Vec<String>,
        cumulative: Vec<f64>,
        null_rate: f64,
    },
}

pub struct Copula {
    marginals: Vec<Marginal>,
    factor: Vec<Vec<f64>>,
    pub shrinkage: f64,
}

/// Average ranks (1-based) of the non-null values, `None` for nulls.
fn average_ranks(values: &[Option<f64>]) -> Vec<Option<f64>> {
    let mut order: Vec<usize> = (0..values.len()).filter(|&i| values[i].is_some()).collect();
    order.sort_by(|&a, &b| values[a].unwrap().total_cmp(&values[b].unwrap()));
    let mut ranks = vec![None; values.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && values[order[j + 1]] == values[order[i]] {
            j += 1;
        }
        let average = (i + 1 + j + 1) as f64 / 2.0;
        for &k in &order[i..=j] {
            ranks[k] = Some(average);
        }
        i = j + 1;
    }
    ranks
}

/// Normal scores of one column, plus its marginal.
fn fit_column(column: &ColumnData, integer: bool) -> (Marginal, Vec<Option<f64>>) {
    match column {
        ColumnData::Numeric(values) => {
            let mut sorted: Vec<f64> = values.iter().flatten().copied().collect();
            sorted.sort_by(|a, b| a.total_cmp(b));
            let m = sorted.len() as f64;
            let scores = average_ranks(values)
                .into_iter()
                .map(|r| r.map(|r| phi_inv((r - 0.5) / m)))
                .collect();
            let null_rate = 1.0 - m / values.len() as f64;
            let mut distinct = sorted.clone();
            distinct.dedup();
            let discrete = integer && distinct.len() <= DISCRETE_MAX_DISTINCT;
            (
                Marginal::Numeric {
                    sorted,
                    integer,
                    discrete,
                    null_rate,
                },
                scores,
            )
        }
        ColumnData::Categorical(values) => {
            let mut counts: std::collections::BTreeMap<&str, usize> =
                std::collections::BTreeMap::new();
            for v in values.iter().flatten() {
                *counts.entry(v.as_str()).or_insert(0) += 1;
            }
            let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            let total: usize = ranked.iter().map(|(_, c)| c).sum();
            let mut cumulative = Vec::with_capacity(ranked.len());
            let mut running = 0.0;
            let mut midpoints = std::collections::HashMap::new();
            for (name, count) in &ranked {
                let share = *count as f64 / total as f64;
                midpoints.insert(*name, running + share / 2.0);
                running += share;
                cumulative.push(running);
            }
            let scores = values
                .iter()
                .map(|v| v.as_ref().map(|v| phi_inv(midpoints[v.as_str()])))
                .collect();
            let categories = ranked.iter().map(|(n, _)| n.to_string()).collect();
            let null_rate = 1.0 - total as f64 / values.len() as f64;
            (
                Marginal::Categorical {
                    categories,
                    cumulative,
                    null_rate,
                },
                scores,
            )
        }
    }
}

/// Pearson correlation over the rows where both scores exist; 0 when fewer than 3 rows overlap or
/// either column is constant on them.
fn pairwise_correlation(a: &[Option<f64>], b: &[Option<f64>]) -> f64 {
    let pairs: Vec<(f64, f64)> = a
        .iter()
        .zip(b)
        .filter_map(|(x, y)| Some(((*x)?, (*y)?)))
        .collect();
    if pairs.len() < 3 {
        return 0.0;
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
    if saa <= 1e-12 || sbb <= 1e-12 {
        0.0
    } else {
        (sab / (saa.sqrt() * sbb.sqrt())).clamp(-1.0, 1.0)
    }
}

impl Copula {
    /// `integer_flags[j]` says whether numeric column `j` holds whole numbers (sampled values are
    /// then rounded).
    pub fn fit(columns: &[ColumnData], integer_flags: &[bool]) -> Copula {
        let (marginals, scores): (Vec<Marginal>, Vec<Vec<Option<f64>>>) = columns
            .iter()
            .zip(integer_flags)
            .map(|(c, &i)| fit_column(c, i))
            .unzip();
        let k = columns.len();
        let mut correlation = vec![vec![0.0; k]; k];
        for i in 0..k {
            correlation[i][i] = 1.0;
            for j in 0..i {
                let r = pairwise_correlation(&scores[i], &scores[j]);
                correlation[i][j] = r;
                correlation[j][i] = r;
            }
        }
        let (factor, shrinkage) = repair_and_factor(&correlation);
        Copula {
            marginals,
            factor,
            shrinkage,
        }
    }

    pub fn sample(&self, rows: usize, rng: &mut Rng) -> Vec<ColumnData> {
        let k = self.marginals.len();
        let mut numeric: Vec<Vec<Option<f64>>> = vec![Vec::with_capacity(rows); k];
        let mut categorical: Vec<Vec<Option<String>>> = vec![Vec::with_capacity(rows); k];
        for _ in 0..rows {
            let noise: Vec<f64> = (0..k).map(|_| rng.normal()).collect();
            for (j, marginal) in self.marginals.iter().enumerate() {
                let z: f64 = (0..=j).map(|m| self.factor[j][m] * noise[m]).sum();
                let u = phi(z);
                let is_null = rng.uniform() < marginal_null_rate(marginal);
                match marginal {
                    Marginal::Numeric {
                        sorted,
                        integer,
                        discrete,
                        ..
                    } => {
                        numeric[j].push((!is_null).then(|| {
                            if *discrete {
                                sorted[((u * sorted.len() as f64) as usize).min(sorted.len() - 1)]
                            } else {
                                quantile(sorted, u, *integer)
                            }
                        }));
                    }
                    Marginal::Categorical {
                        categories,
                        cumulative,
                        ..
                    } => {
                        let index = pick_category(cumulative, u);
                        categorical[j].push((!is_null).then(|| categories[index].clone()));
                    }
                }
            }
        }
        self.marginals
            .iter()
            .enumerate()
            .map(|(j, m)| match m {
                Marginal::Numeric { .. } => ColumnData::Numeric(std::mem::take(&mut numeric[j])),
                Marginal::Categorical { .. } => {
                    ColumnData::Categorical(std::mem::take(&mut categorical[j]))
                }
            })
            .collect()
    }
}

/// Cholesky factor of a correlation matrix, first shrinking it toward the identity (0.01, then
/// doubling) if it is not positive definite. Returns the factor and the shrinkage used.
pub(super) fn repair_and_factor(correlation: &[Vec<f64>]) -> (Vec<Vec<f64>>, f64) {
    let k = correlation.len();
    let mut shrinkage = 0.0;
    loop {
        let shrunk: Vec<Vec<f64>> = (0..k)
            .map(|i| {
                (0..k)
                    .map(|j| {
                        if i == j {
                            1.0
                        } else {
                            (1.0 - shrinkage) * correlation[i][j]
                        }
                    })
                    .collect()
            })
            .collect();
        if let Some(l) = cholesky(&shrunk) {
            return (l, shrinkage);
        }
        shrinkage = next_shrinkage(shrinkage);
    }
}

/// Shrinkage schedule for repairing a non-positive-definite correlation matrix: 0.01, then doubling,
/// capped at 1 (the identity).
fn next_shrinkage(current: f64) -> f64 {
    if current == 0.0 {
        0.01
    } else {
        (current * 2.0_f64).min(1.0)
    }
}

/// The category whose cumulative-frequency interval contains `u`; the last one if rounding left
/// `u` just above the final cumulative value.
fn pick_category(cumulative: &[f64], u: f64) -> usize {
    cumulative
        .iter()
        .position(|&c| u <= c)
        .unwrap_or(cumulative.len() - 1)
}

fn marginal_null_rate(m: &Marginal) -> f64 {
    match m {
        Marginal::Numeric { null_rate, .. } | Marginal::Categorical { null_rate, .. } => *null_rate,
    }
}

/// Empirical quantile with linear interpolation between order statistics.
fn quantile(sorted: &[f64], u: f64, integer: bool) -> f64 {
    let position = u.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let (lo, hi) = (position.floor() as usize, position.ceil() as usize);
    let value = sorted[lo] + (sorted[hi] - sorted[lo]) * (position - lo as f64);
    if integer {
        value.round()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric(values: &[f64]) -> ColumnData {
        ColumnData::Numeric(values.iter().map(|v| Some(*v)).collect())
    }

    #[test]
    fn the_shrinkage_schedule_starts_small_doubles_and_caps_at_one() {
        assert_eq!(next_shrinkage(0.0), 0.01);
        assert_eq!(next_shrinkage(0.01), 0.02);
        assert_eq!(next_shrinkage(0.3), 0.6);
        assert_eq!(next_shrinkage(0.6), 1.0);
        assert_eq!(next_shrinkage(1.0), 1.0);
    }

    #[test]
    fn a_category_is_picked_by_its_cumulative_interval_with_a_last_category_fallback() {
        let cumulative = [0.5, 0.8, 1.0];
        assert_eq!(pick_category(&cumulative, 0.0), 0);
        assert_eq!(
            pick_category(&cumulative, 0.5),
            0,
            "the upper edge belongs to the lower category"
        );
        assert_eq!(pick_category(&cumulative, 0.51), 1);
        assert_eq!(pick_category(&cumulative, 0.99), 2);
        assert_eq!(
            pick_category(&[0.4, 0.999_999_9], 1.0),
            1,
            "u above the last edge falls back to the last category"
        );
    }

    #[test]
    fn numeric_scores_are_the_normal_quantiles_of_the_mid_ranks() {
        let (_, scores) = fit_column(&numeric(&[10.0, 20.0, 30.0, 40.0]), false);
        for (score, u) in scores.iter().zip([0.125, 0.375, 0.625, 0.875]) {
            assert!(
                (score.unwrap() - phi_inv(u)).abs() < 1e-12,
                "{score:?} vs {u}"
            );
        }
    }

    #[test]
    fn categorical_scores_use_the_midpoint_of_each_cumulative_interval() {
        // "a" is 70% of rows (interval 0-0.7, midpoint 0.35), "b" is 30% (0.7-1, midpoint 0.85).
        let values: Vec<Option<String>> = (0..10)
            .map(|i| Some(if i < 7 { "a" } else { "b" }.to_string()))
            .collect();
        let (_, scores) = fit_column(&ColumnData::Categorical(values), false);
        assert!((scores[0].unwrap() - phi_inv(0.35)).abs() < 1e-12);
        assert!((scores[9].unwrap() - phi_inv(0.85)).abs() < 1e-12);
    }

    #[test]
    fn a_categorical_null_rate_is_recorded_and_reproduced() {
        let values: Vec<Option<String>> = (0..1000)
            .map(|i| (i % 5 != 0).then(|| ["x", "y"][i % 2].to_string()))
            .collect();
        let (marginal, _) = fit_column(&ColumnData::Categorical(values.clone()), false);
        assert!((marginal_null_rate(&marginal) - 0.2).abs() < 1e-12);
        let out = Copula::fit(&[ColumnData::Categorical(values)], &[false])
            .sample(20_000, &mut Rng::new(2));
        let ColumnData::Categorical(v) = &out[0] else {
            panic!()
        };
        let nulls = v.iter().filter(|x| x.is_none()).count() as f64 / v.len() as f64;
        assert!((nulls - 0.2).abs() < 0.02, "null rate {nulls}");
    }

    #[test]
    fn pairwise_correlation_has_the_right_sign_size_and_degenerate_cases() {
        let some = |v: &[f64]| v.iter().map(|x| Some(*x)).collect::<Vec<_>>();
        let (x, up, down) = (
            some(&[1.0, 2.0, 3.0, 4.0]),
            some(&[10.0, 20.0, 30.0, 40.0]),
            some(&[4.0, 3.0, 2.0, 1.0]),
        );
        assert!((pairwise_correlation(&x, &up) - 1.0).abs() < 1e-12);
        assert!((pairwise_correlation(&x, &down) + 1.0).abs() < 1e-12);
        // hand value: cov 3, variances 5 and 5 -> 0.6
        assert!((pairwise_correlation(&x, &some(&[2.0, 1.0, 4.0, 3.0])) - 0.6).abs() < 1e-12);
        assert_eq!(
            pairwise_correlation(&x, &some(&[7.0; 4])),
            0.0,
            "a constant column has no correlation"
        );
        assert_eq!(
            pairwise_correlation(&some(&[7.0; 4]), &x),
            0.0,
            "either side constant"
        );
        assert_eq!(
            pairwise_correlation(&some(&[1.0, 2.0]), &some(&[1.0, 2.0])),
            0.0,
            "two pairs are too few"
        );
        assert!(
            (pairwise_correlation(&some(&[1.0, 2.0, 3.0]), &some(&[1.0, 2.0, 3.0])) - 1.0).abs()
                < 1e-12,
            "three pairs are enough"
        );
    }

    #[test]
    fn a_moderate_correlation_is_recovered_not_amplified() {
        let mut rng = Rng::new(21);
        let x: Vec<f64> = (0..3000).map(|_| rng.normal()).collect();
        let y: Vec<f64> = x
            .iter()
            .map(|v| 0.5 * v + 0.75_f64.sqrt() * rng.normal())
            .collect();
        let out = Copula::fit(&[numeric(&x), numeric(&y)], &[false, false])
            .sample(6000, &mut Rng::new(4));
        let (ColumnData::Numeric(a), ColumnData::Numeric(b)) = (&out[0], &out[1]) else {
            panic!()
        };
        let r = pairwise_correlation(a, b);
        assert!((r - 0.5).abs() < 0.05, "expected about 0.5, got {r}");
    }

    #[test]
    fn is_empty_reflects_the_length() {
        assert!(ColumnData::Numeric(vec![]).is_empty());
        assert!(!numeric(&[1.0]).is_empty());
        assert!(!ColumnData::Categorical(vec![None]).is_empty());
    }

    #[test]
    fn average_ranks_share_ties() {
        let ranks = average_ranks(&[Some(3.0), Some(1.0), Some(3.0), None, Some(2.0)]);
        assert_eq!(
            ranks,
            vec![Some(3.5), Some(1.0), Some(3.5), None, Some(2.0)]
        );
    }

    #[test]
    fn a_perfectly_correlated_pair_stays_strongly_correlated_in_the_output() {
        let x: Vec<f64> = (0..200).map(f64::from).collect();
        let y: Vec<f64> = x.iter().map(|v| v * 3.0 + 5.0).collect();
        let copula = Copula::fit(&[numeric(&x), numeric(&y)], &[false, false]);
        let out = copula.sample(2000, &mut Rng::new(3));
        let (ColumnData::Numeric(a), ColumnData::Numeric(b)) = (&out[0], &out[1]) else {
            panic!()
        };
        let (a, b): (Vec<Option<f64>>, Vec<Option<f64>>) = (a.clone(), b.clone());
        assert!(pairwise_correlation(&a, &b) > 0.95);
    }

    #[test]
    fn an_independent_pair_stays_near_uncorrelated() {
        let mut rng = Rng::new(11);
        let x: Vec<f64> = (0..500).map(|_| rng.normal()).collect();
        let y: Vec<f64> = (0..500).map(|_| rng.normal()).collect();
        let copula = Copula::fit(&[numeric(&x), numeric(&y)], &[false, false]);
        let out = copula.sample(3000, &mut Rng::new(5));
        let (ColumnData::Numeric(a), ColumnData::Numeric(b)) = (&out[0], &out[1]) else {
            panic!()
        };
        assert!(pairwise_correlation(a, b).abs() < 0.1);
    }

    #[test]
    fn sampled_values_never_leave_the_observed_range() {
        let x = numeric(&[10.0, 20.0, 30.0, 40.0]);
        let out = Copula::fit(&[x], &[false]).sample(500, &mut Rng::new(1));
        let ColumnData::Numeric(v) = &out[0] else {
            panic!()
        };
        assert!(v.iter().flatten().all(|&x| (10.0..=40.0).contains(&x)));
    }

    #[test]
    fn integer_columns_stay_whole_numbers() {
        let out = Copula::fit(&[numeric(&[1.0, 2.0, 4.0, 9.0, 12.0])], &[true])
            .sample(200, &mut Rng::new(1));
        let ColumnData::Numeric(v) = &out[0] else {
            panic!()
        };
        assert!(v.iter().flatten().all(|x| x.fract() == 0.0));
    }

    #[test]
    fn a_discrete_integer_column_only_produces_observed_values() {
        // zip-like codes: interpolation would invent values between them that never occurred.
        let codes = [10001.0, 10001.0, 94110.0, 94110.0, 94110.0, 60601.0];
        let out = Copula::fit(&[numeric(&codes)], &[true]).sample(2000, &mut Rng::new(1));
        let ColumnData::Numeric(v) = &out[0] else {
            panic!()
        };
        assert!(
            v.iter().flatten().all(|x| codes.contains(x)),
            "an unobserved value was generated"
        );
        let share = v.iter().flatten().filter(|&&x| x == 94110.0).count() as f64 / v.len() as f64;
        assert!((share - 0.5).abs() < 0.04, "share of 94110: {share}");
    }

    #[test]
    fn a_continuous_column_still_interpolates_between_observations() {
        let out = Copula::fit(&[numeric(&[1.0, 2.0, 3.0, 4.0, 5.0])], &[false])
            .sample(500, &mut Rng::new(1));
        let ColumnData::Numeric(v) = &out[0] else {
            panic!()
        };
        assert!(v.iter().flatten().any(|x| x.fract() != 0.0));
    }

    #[test]
    fn categories_come_out_in_roughly_their_original_proportions() {
        let values: Vec<Option<String>> = (0..1000)
            .map(|i| Some(if i % 10 < 7 { "a" } else { "b" }.to_string()))
            .collect();
        let out = Copula::fit(&[ColumnData::Categorical(values)], &[false])
            .sample(20_000, &mut Rng::new(9));
        let ColumnData::Categorical(v) = &out[0] else {
            panic!()
        };
        let a = v.iter().flatten().filter(|s| s.as_str() == "a").count() as f64 / v.len() as f64;
        assert!((a - 0.7).abs() < 0.02, "share of a: {a}");
    }

    #[test]
    fn null_rate_is_preserved() {
        let values: Vec<Option<f64>> = (0..1000)
            .map(|i| (i % 4 != 0).then_some(f64::from(i)))
            .collect();
        let out =
            Copula::fit(&[ColumnData::Numeric(values)], &[false]).sample(20_000, &mut Rng::new(9));
        let ColumnData::Numeric(v) = &out[0] else {
            panic!()
        };
        let nulls = v.iter().filter(|x| x.is_none()).count() as f64 / v.len() as f64;
        assert!((nulls - 0.25).abs() < 0.02, "null rate {nulls}");
    }

    #[test]
    fn the_same_seed_gives_the_same_output() {
        let c = Copula::fit(&[numeric(&[1.0, 2.0, 3.0, 4.0, 5.0])], &[false]);
        assert_eq!(
            c.sample(50, &mut Rng::new(4)),
            c.sample(50, &mut Rng::new(4))
        );
        assert_ne!(
            c.sample(50, &mut Rng::new(4)),
            c.sample(50, &mut Rng::new(5))
        );
    }

    #[test]
    fn duplicate_columns_yield_a_valid_factor_by_shrinkage() {
        // two identical columns: the correlation matrix is singular until shrunk.
        let x = numeric(&(0..50).map(f64::from).collect::<Vec<_>>());
        let copula = Copula::fit(&[x.clone(), x], &[false, false]);
        assert!(copula.shrinkage > 0.0);
        assert_eq!(copula.sample(10, &mut Rng::new(1)).len(), 2);
    }
}
