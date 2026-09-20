//! Small numeric building blocks for synthesis, written in-crate so the generated data is
//! reproducible from a seed on every platform and the crate takes no new dependency: a
//! SplitMix64 PRNG, the standard normal CDF and its inverse, and a Cholesky factorization.

/// SplitMix64: tiny, fast, statistically adequate for sampling, and fully deterministic.
pub struct Rng {
    state: u64,
    spare_normal: Option<f64>,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng {
            state: seed,
            spare_normal: None,
        }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in the open interval (0, 1), never exactly 0 or 1.
    pub fn uniform(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / 9_007_199_254_740_992.0
    }

    /// Standard normal via Box-Muller (the second value of each pair is kept for the next call).
    pub fn normal(&mut self) -> f64 {
        if let Some(z) = self.spare_normal.take() {
            return z;
        }
        let (u1, u2) = (self.uniform(), self.uniform());
        let radius = (-2.0 * u1.ln()).sqrt();
        let angle = 2.0 * std::f64::consts::PI * u2;
        self.spare_normal = Some(radius * angle.sin());
        radius * angle.cos()
    }
}

/// erfc for x >= 0: Taylor series of erf below 1 (where erfc is still large, so `1 - erf` does not
/// cancel), continued fraction from 1 up (accurate in relative terms in the tail).
fn erfc_nonneg(x: f64) -> f64 {
    if x < 1.0 {
        let (mut term, mut sum) = (x, x);
        let x2 = x * x;
        let mut n = 0.0;
        while term.abs() > 1e-17 * sum.abs() {
            n += 1.0;
            term *= -x2 / n;
            sum += term / (2.0 * n + 1.0);
        }
        1.0 - 2.0 / std::f64::consts::PI.sqrt() * sum
    } else {
        // erfc(x) = exp(-x^2)/sqrt(pi) * 1/(x + (1/2)/(x + 1/(x + (3/2)/(x + ...))))
        let mut fraction = x;
        let terms = if x < 2.0 { 150 } else { 60 };
        for k in (1..=terms).rev() {
            fraction = x + (k as f64 / 2.0) / fraction;
        }
        (-x * x).exp() / (std::f64::consts::PI.sqrt() * fraction)
    }
}

/// Standard normal CDF.
pub fn phi(x: f64) -> f64 {
    let z = x / std::f64::consts::SQRT_2;
    if z >= 0.0 {
        1.0 - 0.5 * erfc_nonneg(z)
    } else {
        0.5 * erfc_nonneg(-z)
    }
}

/// Inverse standard normal CDF: Acklam's rational approximation (lower half only, the upper half
/// is reflected) refined by one Halley step against `phi`. `p` is clamped into the open interval
/// so callers may pass 0 or 1.
pub fn phi_inv(p: f64) -> f64 {
    let p = p.clamp(1e-15, 1.0 - 1e-15);
    // Solve in the lower tail and reflect: `phi(x) - p` cancels completely for p near 1, but is
    // accurate for p near 0, and 1 - p is exact in floating point there.
    if p > 0.5 {
        -phi_inv_lower(1.0 - p)
    } else {
        phi_inv_lower(p)
    }
}

fn phi_inv_lower(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.38357751867269e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];
    const P_LOW: f64 = 0.02425;
    let x = if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    };
    let error = phi(x) - p;
    let u = error * (2.0 * std::f64::consts::PI).sqrt() * (x * x / 2.0).exp();
    x - u / (1.0 + x * u / 2.0)
}

/// Lower-triangular Cholesky factor `L` with `L * L^T = matrix`, or `None` if the matrix is not
/// positive definite.
pub fn cholesky(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = matrix.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let sum: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
            if i == j {
                let d = matrix[i][i] - sum;
                if d <= 1e-12 {
                    return None;
                }
                l[i][i] = d.sqrt();
            } else {
                l[i][j] = (matrix[i][j] - sum) / l[j][j];
            }
        }
    }
    Some(l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_reference_output() {
        // Reference values from an independent implementation (Python) of Vigna's SplitMix64;
        // seed 0 also matches the published test vector.
        let mut r = Rng::new(0);
        assert_eq!(
            [r.next_u64(), r.next_u64(), r.next_u64()],
            [
                0xe220_a839_7b1d_cdaf,
                0x6e78_9e6a_a1b9_65f4,
                0x06c4_5d18_8009_454f
            ]
        );
        let mut r = Rng::new(7);
        assert_eq!(
            [r.next_u64(), r.next_u64()],
            [0x63cb_e1e4_5932_0dd7, 0x044c_3cd7_f43c_661c]
        );
    }

    #[test]
    fn the_first_uniform_for_a_seed_is_the_reference_value() {
        assert_eq!(Rng::new(7).uniform(), 0.38982974839127155);
    }

    #[test]
    fn phi_inv_matches_reference_quantiles_to_machine_precision() {
        // Python statistics.NormalDist().inv_cdf (Wichura AS241), a different algorithm.
        assert!((phi_inv(0.975) - 1.9599639845400536).abs() < 1e-13);
        assert!((phi_inv(0.3) + 0.5244005127080407).abs() < 1e-13);
        assert!((phi_inv(0.001) + 3.090232306167813).abs() < 1e-13);
    }

    #[test]
    fn phi_inv_clamps_the_extremes_to_finite_values() {
        // The two clamp bounds (1e-15 and 1 - 1e-15) are not exactly symmetric in floating point,
        // so their quantiles differ slightly; reference values from Python's NormalDist.
        let (low, high) = (phi_inv(0.0), phi_inv(1.0));
        assert!((low + 7.941345326170997).abs() < 1e-9, "{low}");
        assert!((high - 7.941444487415977).abs() < 1e-9, "{high}");
    }

    #[test]
    fn phi_inv_round_trips_to_high_precision_where_the_density_is_not_tiny() {
        for i in -40..=40 {
            let x = f64::from(i) / 10.0;
            assert!((phi_inv(phi(x)) - x).abs() < 1e-11, "x = {x}");
        }
    }

    #[test]
    fn the_prng_is_deterministic_and_seed_sensitive() {
        let (mut a, mut b, mut c) = (Rng::new(7), Rng::new(7), Rng::new(8));
        let xs: Vec<f64> = (0..5).map(|_| a.uniform()).collect();
        assert_eq!(xs, (0..5).map(|_| b.uniform()).collect::<Vec<_>>());
        assert_ne!(xs, (0..5).map(|_| c.uniform()).collect::<Vec<_>>());
    }

    #[test]
    fn uniforms_stay_strictly_inside_the_unit_interval_and_look_uniform() {
        let mut rng = Rng::new(1);
        let xs: Vec<f64> = (0..100_000).map(|_| rng.uniform()).collect();
        assert!(xs.iter().all(|&x| x > 0.0 && x < 1.0));
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        assert!((mean - 0.5).abs() < 0.005, "mean {mean}");
    }

    #[test]
    fn normals_have_mean_zero_and_unit_variance() {
        let mut rng = Rng::new(2);
        let xs: Vec<f64> = (0..200_000).map(|_| rng.normal()).collect();
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / xs.len() as f64;
        assert!(mean.abs() < 0.01, "mean {mean}");
        assert!((var - 1.0).abs() < 0.02, "variance {var}");
    }

    #[test]
    fn phi_matches_known_values() {
        assert!((phi(0.0) - 0.5).abs() < 1e-15);
        assert!((phi(1.96) - 0.9750021048517795).abs() < 1e-12);
        assert!((phi(-1.0) - 0.15865525393145707).abs() < 1e-12);
        assert!(
            (phi(4.0) - 0.9999683287581669).abs() < 1e-12,
            "the tail branch"
        );
        assert!(
            (phi(-5.0) - 2.866515718791939e-7).abs() < 1e-18,
            "a far tail"
        );
    }

    #[test]
    fn phi_keeps_relative_precision_in_both_tails() {
        // reference: Python statistics.NormalDist().cdf, relative tolerance 2e-14
        for (x, want) in [
            (-1.0, 0.15865525393145707),
            (-2.0, 0.022750131948179195),
            (-3.0, 0.0013498980316301035),
            (-4.0, 3.167124183311992e-05),
            (-6.0, 9.865876450376946e-10),
        ] {
            assert!(
                ((phi(x) - want) / want).abs() < 2e-14,
                "phi({x}) = {:e}, want {want:e}",
                phi(x)
            );
        }
        assert!(((1.0 - phi(5.0)) - 2.866515718791939e-7).abs() < 1e-15);
    }

    #[test]
    fn phi_inv_matches_known_values() {
        assert!((phi_inv(0.975) - 1.959963984540054).abs() < 1e-9);
        assert!((phi_inv(0.5)).abs() < 1e-12);
        assert!(
            (phi_inv(0.001) + 3.090232306167813).abs() < 1e-8,
            "the low tail branch"
        );
    }

    #[test]
    fn phi_inv_inverts_phi_across_the_range() {
        for i in -60..=60 {
            let x = f64::from(i) / 10.0;
            assert!((phi_inv(phi(x)) - x).abs() < 1e-7, "x = {x}");
        }
    }

    #[test]
    fn cholesky_factors_a_positive_definite_matrix_and_rejects_others() {
        let m = vec![vec![4.0, 2.0], vec![2.0, 3.0]];
        let l = cholesky(&m).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                let product: f64 = (0..2).map(|k| l[i][k] * l[j][k]).sum();
                assert!((product - m[i][j]).abs() < 1e-12);
            }
        }
        assert!(
            cholesky(&[vec![1.0, 2.0], vec![2.0, 1.0]]).is_none(),
            "indefinite"
        );
    }
}
