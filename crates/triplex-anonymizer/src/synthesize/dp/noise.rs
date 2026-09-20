//! Differential-privacy noise: an exact discrete Laplace sampler driven by integer arithmetic.
//!
//! A textbook Laplace mechanism that samples an `f64` is not epsilon-DP as implemented: the
//! floating-point representation lets an observer tell neighboring datasets apart (Mironov, 2012).
//! Every statistic released here is an integer count, so the noise is drawn from the discrete
//! Laplace distribution with the exact sampler of Canonne, Kamath and Steinke ("The Discrete
//! Gaussian for Differential Privacy", 2020, Algorithms 1 and 2), which uses only integer
//! comparisons and Bernoulli draws with rational parameters.
//!
//! The bits come from a [`NoiseSource`]. Production uses [`OsNoise`], the operating system's
//! secure random source; [`InsecureSeededNoise`] exists so tests are reproducible and must never
//! back a real release.

use super::super::numeric::Rng;
use triplex_core::{Error, Result};

pub trait NoiseSource {
    fn next_u64(&mut self) -> u64;
}

/// The operating system's cryptographically secure random source.
pub struct OsNoise;

impl NoiseSource for OsNoise {
    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        getrandom::getrandom(&mut bytes).expect("the operating system's random source failed");
        u64::from_le_bytes(bytes)
    }
}

/// Predictable noise for tests. **Insecure: a release made with it carries no privacy guarantee.**
pub struct InsecureSeededNoise(Rng);

impl InsecureSeededNoise {
    pub fn new(seed: u64) -> Self {
        InsecureSeededNoise(Rng::new(seed))
    }
}

impl NoiseSource for InsecureSeededNoise {
    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }
}

/// Uniform integer in `0..n`, without modulo bias (rejects the low values that would skew it).
fn below(source: &mut dyn NoiseSource, n: u64) -> u64 {
    debug_assert!(n > 0);
    let threshold = n.wrapping_neg() % n;
    loop {
        let x = source.next_u64();
        if x >= threshold {
            return x % n;
        }
    }
}

/// True with probability `numerator / denominator` (exactly, `numerator <= denominator`).
fn bernoulli(source: &mut dyn NoiseSource, numerator: u64, denominator: u64) -> bool {
    below(source, denominator) < numerator
}

/// True with probability exp(-numerator/denominator), for a ratio at most 1 (CKS Algorithm 1).
fn bernoulli_exp_neg_at_most_one(
    source: &mut dyn NoiseSource,
    numerator: u64,
    denominator: u64,
) -> bool {
    let mut k: u64 = 1;
    while bernoulli(source, numerator, denominator * k) {
        k += 1;
    }
    k % 2 == 1
}

/// One draw from the discrete Laplace distribution with P(z) proportional to exp(-|z| * s / t)
/// (CKS Algorithm 2).
fn discrete_laplace_ratio(source: &mut dyn NoiseSource, t: u64, s: u64) -> i64 {
    loop {
        let u = below(source, t);
        if !bernoulli_exp_neg_at_most_one(source, u, t) {
            continue;
        }
        let mut v: u64 = 0;
        while bernoulli_exp_neg_at_most_one(source, 1, 1) {
            v += 1;
        }
        let y = u.saturating_add(t.saturating_mul(v)) / s;
        let negative = below(source, 2) == 1;
        if negative && y == 0 {
            continue;
        }
        let y = i64::try_from(y).unwrap_or(i64::MAX);
        return if negative { -y } else { y };
    }
}

/// Resolution at which epsilon is rounded down: the noise is calibrated to
/// `floor(epsilon * 1e6) / 1e6`, never more than the requested budget.
pub const EPSILON_RESOLUTION: u64 = 1_000_000;

/// The epsilon the noise is actually calibrated to for a requested `epsilon`: rounded down to
/// [`EPSILON_RESOLUTION`], so the noise is at least as large as the request implies.
pub fn effective_epsilon(epsilon: f64) -> f64 {
    (epsilon * EPSILON_RESOLUTION as f64).floor() / EPSILON_RESOLUTION as f64
}

/// Adds discrete-Laplace noise calibrated for a query with integer L1 `sensitivity` under
/// `epsilon`. The result is epsilon-differentially private for that one query.
pub fn add_noise(
    source: &mut dyn NoiseSource,
    value: i64,
    sensitivity: u64,
    epsilon: f64,
) -> Result<i64> {
    if !(epsilon.is_finite() && epsilon > 0.0) || sensitivity == 0 {
        return Err(Error::Validation(
            "epsilon must be a positive number and the sensitivity at least 1".into(),
        ));
    }
    let s = (epsilon * EPSILON_RESOLUTION as f64).floor() as u64;
    if s == 0 {
        return Err(Error::Validation(format!(
            "epsilon {epsilon} is below the resolution {:e} of the noise sampler",
            1.0 / EPSILON_RESOLUTION as f64
        )));
    }
    let t = sensitivity.checked_mul(EPSILON_RESOLUTION).ok_or_else(|| {
        Error::Validation("sensitivity is too large for the noise sampler".into())
    })?;
    Ok(value.saturating_add(discrete_laplace_ratio(source, t, s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(seed: u64) -> InsecureSeededNoise {
        InsecureSeededNoise::new(seed)
    }

    #[test]
    fn below_is_uniform_and_in_range() {
        let mut src = source(1);
        let mut counts = [0usize; 7];
        for _ in 0..70_000 {
            let x = below(&mut src, 7);
            assert!(x < 7);
            counts[x as usize] += 1;
        }
        for c in counts {
            assert!((c as f64 - 10_000.0).abs() < 400.0, "counts {counts:?}");
        }
    }

    #[test]
    fn bernoulli_has_the_requested_probability() {
        let mut src = source(2);
        let hits = (0..100_000).filter(|_| bernoulli(&mut src, 3, 10)).count() as f64 / 100_000.0;
        assert!((hits - 0.3).abs() < 0.01, "{hits}");
        assert!(
            (0..1000).all(|_| bernoulli(&mut src, 5, 5)),
            "probability 1"
        );
        assert!(
            (0..1000).all(|_| !bernoulli(&mut src, 0, 5)),
            "probability 0"
        );
    }

    #[test]
    fn bernoulli_exp_neg_matches_the_exponential() {
        let mut src = source(3);
        for (num, den) in [(1u64, 1u64), (1, 2), (3, 4), (0, 5)] {
            let hits = (0..200_000)
                .filter(|_| bernoulli_exp_neg_at_most_one(&mut src, num, den))
                .count() as f64
                / 200_000.0;
            let want = (-(num as f64) / den as f64).exp();
            assert!(
                (hits - want).abs() < 0.006,
                "exp(-{num}/{den}): got {hits}, want {want}"
            );
        }
    }

    #[test]
    fn the_discrete_laplace_is_symmetric_with_the_theoretical_variance() {
        // scale t/s = 4 -> alpha = exp(-1/4); Var = 2*alpha / (1-alpha)^2.
        let (t, s) = (4u64, 1u64);
        let alpha = (-(s as f64) / t as f64).exp();
        let want_variance = 2.0 * alpha / (1.0 - alpha).powi(2);
        let mut src = source(4);
        let draws: Vec<f64> = (0..400_000)
            .map(|_| discrete_laplace_ratio(&mut src, t, s) as f64)
            .collect();
        let mean = draws.iter().sum::<f64>() / draws.len() as f64;
        let variance = draws.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / draws.len() as f64;
        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!(
            ((variance - want_variance) / want_variance).abs() < 0.03,
            "variance {variance}, want {want_variance}"
        );
    }

    #[test]
    fn the_probability_of_zero_matches_the_discrete_laplace_mass() {
        // P(0) = (1 - alpha) / (1 + alpha)
        let (t, s) = (3u64, 1u64);
        let alpha = (-(s as f64) / t as f64).exp();
        let want = (1.0 - alpha) / (1.0 + alpha);
        let mut src = source(5);
        let zeros = (0..300_000)
            .filter(|_| discrete_laplace_ratio(&mut src, t, s) == 0)
            .count() as f64
            / 300_000.0;
        assert!((zeros - want).abs() < 0.006, "P(0) = {zeros}, want {want}");
    }

    #[test]
    fn noise_scale_follows_sensitivity_over_epsilon() {
        // sensitivity 2, epsilon 1 -> scale 2; sensitivity 2, epsilon 0.5 -> scale 4 (twice as noisy).
        let spread = |eps: f64| {
            let mut src = source(6);
            let draws: Vec<f64> = (0..100_000)
                .map(|_| add_noise(&mut src, 0, 2, eps).unwrap() as f64)
                .collect();
            draws.iter().map(|x| x.abs()).sum::<f64>() / draws.len() as f64
        };
        let ratio = spread(0.5) / spread(1.0);
        assert!((ratio - 2.0).abs() < 0.1, "noise ratio {ratio}");
    }

    #[test]
    fn epsilon_is_rounded_down_never_up() {
        assert_eq!(effective_epsilon(1.0), 1.0);
        assert_eq!(effective_epsilon(0.1234567), 0.123456);
        assert!(effective_epsilon(0.3) <= 0.3);
    }

    #[test]
    fn invalid_parameters_are_refused() {
        let mut src = source(7);
        assert!(add_noise(&mut src, 0, 2, 0.0).is_err());
        assert!(add_noise(&mut src, 0, 2, -1.0).is_err());
        assert!(add_noise(&mut src, 0, 2, f64::NAN).is_err());
        assert!(
            add_noise(&mut src, 0, 2, f64::INFINITY).is_err(),
            "an infinite budget is not a budget"
        );
        let message = add_noise(&mut src, 0, 2, 0.0).unwrap_err().to_string();
        assert!(message.contains("positive number"), "{message}");
        assert!(add_noise(&mut src, 0, 0, 1.0).is_err(), "zero sensitivity");
        assert!(
            add_noise(&mut src, 0, 2, 1e-9).is_err(),
            "below the sampler resolution"
        );
    }

    #[test]
    fn os_noise_produces_different_values() {
        let mut os = OsNoise;
        let a: Vec<u64> = (0..4).map(|_| os.next_u64()).collect();
        assert!(a.windows(2).any(|w| w[0] != w[1]));
    }
}
