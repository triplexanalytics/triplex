//! The conditional probability tables of the network and ancestral sampling from them.

use super::structure::Network;
use crate::synthesize::numeric::Rng;

/// P(child cell | parent configuration): one cumulative distribution per configuration.
pub(crate) struct Cpt {
    cdf: Vec<Vec<f64>>,
}

impl Cpt {
    /// `counts[config * child_arity + child_cell]` (exact or noisy; negatives are treated as 0).
    /// A configuration with no mass falls back to uniform over the child's value cells (never the
    /// null cell), a public rule that does not look at the data.
    pub(crate) fn from_counts(counts: &[i64], configs: usize, child_arity: usize) -> Cpt {
        let value_cells = child_arity - 1;
        let cdf = (0..configs)
            .map(|config| {
                let row: Vec<i64> = counts[config * child_arity..(config + 1) * child_arity]
                    .iter()
                    .map(|&c| c.max(0))
                    .collect();
                let total: i64 = row.iter().sum();
                let shares: Vec<f64> = (0..child_arity)
                    .map(|cell| {
                        if total > 0 {
                            row[cell] as f64 / total as f64
                        } else if cell < value_cells {
                            1.0 / value_cells as f64
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let mut running = 0.0;
                let mut cdf: Vec<f64> = shares
                    .iter()
                    .map(|s| {
                        running += s;
                        running
                    })
                    .collect();
                // Exactly 1 from the last cell with any mass on, whatever float rounding did, so
                // a cell with zero probability is never drawn.
                let last = shares.iter().rposition(|&s| s > 0.0).unwrap_or(0);
                for c in &mut cdf[last..] {
                    *c = 1.0;
                }
                cdf
            })
            .collect();
        Cpt { cdf }
    }

    pub(crate) fn draw(&self, config: usize, u: f64) -> usize {
        // Strictly below 1 and compared with `<`, so a cell with zero probability (whose cumulative
        // value equals its predecessor's) is never drawn.
        let u = if u.is_nan() {
            0.5
        } else {
            u.clamp(0.0, 1.0 - f64::EPSILON)
        };
        self.cdf[config]
            .iter()
            .position(|&c| u < c)
            .expect("the last cumulative value is exactly 1 and u is below 1")
    }
}

/// Post-processing for a noisy table (free of privacy cost): a child cell whose mass summed over
/// every parent configuration is no more than the noise alone would produce is set to zero in all
/// configurations. Without it, clamping negatives leaves positive noise in every empty cell, so a
/// sparse table hands out values (the null, `(other)`, empty bins) that the data never had.
///
/// `scale` is the Laplace scale of the noise (`sensitivity / epsilon`). A clamped Laplace draw has
/// mean `scale / 2` and standard deviation about `0.87 * scale`, so a cell is kept only when its
/// total exceeds the noise mean by three standard deviations of the sum.
pub(crate) fn suppress_unsupported_cells(
    counts: &mut [i64],
    configs: usize,
    child_arity: usize,
    scale: f64,
) {
    let expected = configs as f64 * scale / 2.0;
    let margin = 3.0 * 0.87 * scale * (configs as f64).sqrt();
    for cell in 0..child_arity {
        let total: i64 = (0..configs)
            .map(|c| counts[c * child_arity + cell].max(0))
            .sum();
        if (total as f64) <= expected + margin {
            for c in 0..configs {
                counts[c * child_arity + cell] = 0;
            }
        }
    }
}

/// Ancestral sampling: `cells[column][row]` for `rows` records, parents before children.
pub(crate) fn sample(
    network: &Network,
    cpts: &[Cpt],
    arities: &[usize],
    rows: usize,
    rng: &mut Rng,
) -> Vec<Vec<u32>> {
    let mut cells = vec![vec![0u32; rows]; arities.len()];
    #[allow(clippy::needless_range_loop)] // `row` indexes several columns of `cells` at once
    for row in 0..rows {
        for &node in &network.order {
            let config = network.parents[node]
                .iter()
                .fold(0, |acc, &p| acc * arities[p] + cells[p][row] as usize);
            cells[node][row] = cpts[node].draw(config, rng.uniform()) as u32;
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_alone_is_suppressed_and_real_mass_survives() {
        // 4 configurations x 3 child cells, Laplace scale 10. Cell 0 has real mass in every
        // configuration; cells 1 and 2 hold only noise-sized counts.
        let mut counts = vec![
            300, 8, -5, //
            250, 12, 0, //
            280, -3, 14, //
            310, 0, 9,
        ];
        suppress_unsupported_cells(&mut counts, 4, 3, 10.0);
        for config in 0..4 {
            assert!(counts[config * 3] > 200, "the real cell is kept");
            assert_eq!(counts[config * 3 + 1], 0);
            assert_eq!(counts[config * 3 + 2], 0);
        }
    }

    #[test]
    fn a_table_with_no_mass_is_uniform_over_value_cells_never_null() {
        // child has 3 value cells + the null cell; one parent configuration with all-zero counts
        let cpt = Cpt::from_counts(&[0, 0, 0, 0], 1, 4);
        assert_eq!(cpt.draw(0, 0.0), 0);
        assert_eq!(cpt.draw(0, 0.5), 1);
        assert_eq!(cpt.draw(0, 0.99), 2);
        assert_eq!(
            cpt.draw(0, 1.0),
            2,
            "the null cell must have zero probability"
        );
    }

    #[test]
    fn negative_noisy_counts_count_as_zero() {
        let cpt = Cpt::from_counts(&[-5, 10, 0], 1, 3);
        assert_eq!(cpt.draw(0, 0.0), 1);
        assert_eq!(cpt.draw(0, 1.0), 1);
    }

    #[test]
    fn each_parent_configuration_has_its_own_distribution() {
        // config 0 -> always cell 0; config 1 -> always cell 1
        let cpt = Cpt::from_counts(&[10, 0, 0, 0, 10, 0], 2, 3);
        assert_eq!(cpt.draw(0, 0.7), 0);
        assert_eq!(cpt.draw(1, 0.7), 1);
    }
}
