//! Kendall's tau as the dependence statistic for the DP copula.
//!
//! The released integer is `C - D`: over all pairs of rows, the sum of sign(dx) * sign(dy), where
//! a pair with a null on either side contributes 0. Replacing one record changes each of the
//! n - 1 pairs it belongs to by at most 2 (from +1 to -1), so the L1 sensitivity is 2(n - 1).

use super::noise::{add_noise, NoiseSource};
use std::f64::consts::PI;
use triplex_core::Result;

/// Sensitivity of `concordance_score` for `n` rows.
pub fn sensitivity(n: usize) -> u64 {
    2 * (n as u64).saturating_sub(1)
}

fn sign(a: f64, b: f64) -> i64 {
    if a > b {
        1
    } else if a < b {
        -1
    } else {
        0
    }
}

/// `C - D` over all row pairs (O(n^2)). Values are expected to be clamped to their declared bounds
/// already; nulls (`None`) make a pair contribute 0.
pub fn concordance_score(x: &[Option<f64>], y: &[Option<f64>]) -> i64 {
    let n = x.len().min(y.len());
    let mut score = 0i64;
    for i in 0..n {
        let (Some(xi), Some(yi)) = (x[i], y[i]) else {
            continue;
        };
        for j in (i + 1)..n {
            if let (Some(xj), Some(yj)) = (x[j], y[j]) {
                score += sign(xi, xj) * sign(yi, yj);
            }
        }
    }
    score
}

/// A noisy correlation for one column pair, and nothing about the data beyond that.
/// `tau = noisy(C - D) / (n(n-1)/2)`, clipped to [-1, 1]; `rho = sin(pi * tau / 2)`.
pub fn noisy_correlation(
    x: &[Option<f64>],
    y: &[Option<f64>],
    epsilon: f64,
    source: &mut dyn NoiseSource,
) -> Result<f64> {
    let n = x.len().min(y.len());
    let noisy = add_noise(source, concordance_score(x, y), sensitivity(n), epsilon)?;
    Ok(correlation_from_score(noisy, n))
}

pub fn correlation_from_score(score: i64, n: usize) -> f64 {
    let pairs = (n * n.saturating_sub(1)) as f64 / 2.0;
    if pairs == 0.0 {
        return 0.0;
    }
    let tau = (score as f64 / pairs).clamp(-1.0, 1.0);
    (PI * tau / 2.0).sin()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthesize::dp::noise::InsecureSeededNoise;

    fn some(v: &[f64]) -> Vec<Option<f64>> {
        v.iter().map(|x| Some(*x)).collect()
    }

    #[test]
    fn a_monotone_pair_is_fully_concordant_and_a_reversed_one_fully_discordant() {
        let x = some(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(concordance_score(&x, &some(&[10.0, 20.0, 30.0, 40.0])), 6);
        assert_eq!(concordance_score(&x, &some(&[4.0, 3.0, 2.0, 1.0])), -6);
    }

    #[test]
    fn ties_and_nulls_contribute_zero() {
        // pairs: (0,1) tie in x -> 0; (0,2) +1; (1,2) +1 -> 2
        assert_eq!(
            concordance_score(&some(&[1.0, 1.0, 2.0]), &some(&[1.0, 2.0, 3.0])),
            2
        );
        let with_null = vec![Some(1.0), None, Some(3.0)];
        assert_eq!(
            concordance_score(&with_null, &some(&[1.0, 2.0, 3.0])),
            1,
            "only the pair (0,2) counts"
        );
    }

    #[test]
    fn the_score_is_symmetric_in_its_arguments() {
        let (x, y) = (some(&[3.0, 1.0, 2.0, 5.0]), some(&[1.0, 4.0, 2.0, 3.0]));
        assert_eq!(concordance_score(&x, &y), concordance_score(&y, &x));
    }

    #[test]
    fn replacing_one_record_changes_the_score_by_at_most_2_n_minus_1() {
        // the sensitivity the noise is calibrated to, checked as a property over many datasets
        let mut src = InsecureSeededNoise::new(3);
        let mut next = |bound: u64| {
            use crate::synthesize::dp::noise::NoiseSource;
            src.next_u64() % bound
        };
        for _ in 0..200 {
            let n = 4 + next(20) as usize;
            let x: Vec<Option<f64>> = (0..n)
                .map(|_| (next(10) != 0).then(|| next(6) as f64))
                .collect();
            let y: Vec<Option<f64>> = (0..n)
                .map(|_| (next(10) != 0).then(|| next(6) as f64))
                .collect();
            let base = concordance_score(&x, &y);
            for _ in 0..10 {
                let victim = next(n as u64) as usize;
                let (mut x2, mut y2) = (x.clone(), y.clone());
                x2[victim] = (next(5) != 0).then(|| next(6) as f64);
                y2[victim] = (next(5) != 0).then(|| next(6) as f64);
                let delta = (concordance_score(&x2, &y2) - base).unsigned_abs();
                assert!(
                    delta <= sensitivity(n),
                    "n={n}: changed by {delta}, bound {}",
                    sensitivity(n)
                );
            }
        }
    }

    #[test]
    fn a_score_becomes_a_correlation_with_the_right_end_points() {
        assert_eq!(correlation_from_score(6, 4), 1.0, "tau 1 -> rho 1");
        assert_eq!(correlation_from_score(-6, 4), -1.0);
        assert_eq!(correlation_from_score(0, 4), 0.0);
        // tau 0.5 -> rho = sin(pi/4)
        assert!((correlation_from_score(3, 4) - (PI / 4.0).sin()).abs() < 1e-12);
        assert_eq!(
            correlation_from_score(1000, 4),
            1.0,
            "noisy scores are clipped"
        );
        assert_eq!(correlation_from_score(5, 1), 0.0, "no pairs");
    }

    #[test]
    fn the_noisy_correlation_is_close_to_the_true_one_at_a_generous_epsilon() {
        let n = 400;
        let x: Vec<Option<f64>> = (0..n).map(|i| Some(f64::from(i))).collect();
        let y: Vec<Option<f64>> = (0..n)
            .map(|i| Some(f64::from(i) + f64::from((i * 37) % 91)))
            .collect();
        let exact = correlation_from_score(concordance_score(&x, &y), n as usize);
        let mut src = InsecureSeededNoise::new(2);
        let noisy = noisy_correlation(&x, &y, 5.0, &mut src).unwrap();
        assert!(
            (noisy - exact).abs() < 0.05,
            "noisy {noisy} vs exact {exact}"
        );
    }
}
