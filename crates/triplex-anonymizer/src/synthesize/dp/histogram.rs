//! Noisy histograms: the marginal mechanism.
//!
//! Every column has a declared, public domain. A record maps to exactly one cell of that domain
//! through a public function (numeric values are clamped into the declared bounds and binned;
//! categorical values outside the declared list become `(other)`; an empty or unparsable cell is
//! the null cell). Replacing one record moves one count from one cell to another, so the L1
//! sensitivity of the count vector is 2.

use super::noise::{add_noise, NoiseSource};
use triplex_core::{Error, Result};

pub const HISTOGRAM_SENSITIVITY: u64 = 2;
pub const OTHER_CATEGORY: &str = "(other)";

#[derive(Debug, Clone, PartialEq)]
pub enum Domain {
    Numeric {
        min: f64,
        max: f64,
        integer: bool,
        bins: usize,
    },
    Categorical {
        values: Vec<String>,
    },
}

impl Domain {
    /// Number of value cells (excluding the null cell).
    pub fn value_cells(&self) -> usize {
        match self {
            Domain::Numeric { bins, .. } => *bins,
            Domain::Categorical { values } => values.len() + 1, // declared values plus "(other)"
        }
    }

    /// The cell a raw value falls in; the null cell is `value_cells()`.
    pub fn cell(&self, raw: Option<&str>) -> usize {
        let null_cell = self.value_cells();
        let Some(raw) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
            return null_cell;
        };
        match self {
            Domain::Numeric { min, max, bins, .. } => {
                match triplex_core::normalize_numeric_string(raw).parse::<f64>() {
                    Ok(x) if x.is_finite() => {
                        let clamped = x.clamp(*min, *max);
                        (((clamped - min) / (max - min) * *bins as f64) as usize).min(bins - 1)
                    }
                    _ => null_cell,
                }
            }
            Domain::Categorical { values } => {
                values.iter().position(|v| v == raw).unwrap_or(values.len())
            }
        }
    }

    pub fn validate(&self, name: &str) -> Result<()> {
        match self {
            Domain::Numeric { min, max, bins, .. } => {
                if !(min.is_finite() && max.is_finite() && min < max) {
                    return Err(Error::Validation(format!(
                        "column \"{name}\": min must be less than max, both finite"
                    )));
                }
                if *bins < 2 {
                    return Err(Error::Validation(format!(
                        "column \"{name}\": at least 2 bins are needed"
                    )));
                }
            }
            Domain::Categorical { values } => {
                if values.is_empty() {
                    return Err(Error::Validation(format!(
                        "column \"{name}\": declare at least one category value"
                    )));
                }
                let mut seen = std::collections::HashSet::new();
                if values
                    .iter()
                    .any(|v| !seen.insert(v.as_str()) || v == OTHER_CATEGORY)
                {
                    return Err(Error::Validation(format!("column \"{name}\": category values must be distinct and not \"{OTHER_CATEGORY}\"")));
                }
            }
        }
        Ok(())
    }
}

/// Exact counts per cell (value cells then the null cell).
pub fn counts(domain: &Domain, cells: impl Iterator<Item = Option<String>>) -> Vec<i64> {
    let mut counts = vec![0i64; domain.value_cells() + 1];
    for raw in cells {
        counts[domain.cell(raw.as_deref())] += 1;
    }
    counts
}

/// Adds discrete-Laplace noise to every count with the given epsilon, then clamps negatives to
/// zero (post-processing, free of privacy cost).
pub fn noisy_counts(exact: &[i64], epsilon: f64, source: &mut dyn NoiseSource) -> Result<Vec<i64>> {
    exact
        .iter()
        .map(|&c| add_noise(source, c, HISTOGRAM_SENSITIVITY, epsilon).map(|n| n.max(0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthesize::dp::noise::InsecureSeededNoise;

    fn numeric() -> Domain {
        Domain::Numeric {
            min: 0.0,
            max: 100.0,
            integer: false,
            bins: 10,
        }
    }

    #[test]
    fn numeric_values_are_binned_and_clamped_into_the_declared_bounds() {
        let d = numeric();
        assert_eq!(d.cell(Some("0")), 0);
        assert_eq!(d.cell(Some("9.99")), 0);
        assert_eq!(d.cell(Some("10")), 1);
        assert_eq!(d.cell(Some("99.9")), 9);
        assert_eq!(
            d.cell(Some("100")),
            9,
            "the upper bound belongs to the last bin"
        );
        assert_eq!(
            d.cell(Some("-50")),
            0,
            "below the bounds clamps to the first bin"
        );
        assert_eq!(
            d.cell(Some("1e9")),
            9,
            "above the bounds clamps to the last bin"
        );
    }

    #[test]
    fn empty_and_unparsable_values_are_the_null_cell() {
        let d = numeric();
        assert_eq!(d.cell(None), 10);
        assert_eq!(d.cell(Some("  ")), 10);
        assert_eq!(d.cell(Some("abc")), 10);
        assert_eq!(d.cell(Some("NaN")), 10);
    }

    #[test]
    fn categorical_values_outside_the_declared_list_become_other() {
        let d = Domain::Categorical {
            values: vec!["a".into(), "b".into()],
        };
        assert_eq!(
            (
                d.cell(Some("a")),
                d.cell(Some("b")),
                d.cell(Some("zzz")),
                d.cell(None)
            ),
            (0, 1, 2, 3)
        );
        assert_eq!(d.value_cells(), 3);
    }

    #[test]
    fn replacing_one_record_changes_the_counts_by_at_most_two_in_l1() {
        // the sensitivity the noise is calibrated to, checked as a property over many datasets
        let d = numeric();
        let mut src = InsecureSeededNoise::new(9);
        for trial in 0..300u64 {
            let data: Vec<Option<String>> = (0..50)
                .map(|i| Some(((i * 7 + trial as usize * 13) % 140).to_string()))
                .collect();
            let base = counts(&d, data.iter().cloned());
            for replacement in [
                None,
                Some("55".to_string()),
                Some("-3".to_string()),
                Some("x".to_string()),
            ] {
                let mut neighbor = data.clone();
                let victim = (src_index(&mut src, 50)) as usize;
                neighbor[victim] = replacement;
                let other = counts(&d, neighbor.into_iter());
                let l1: i64 = base.iter().zip(&other).map(|(a, b)| (a - b).abs()).sum();
                assert!(l1 <= HISTOGRAM_SENSITIVITY as i64, "L1 {l1}");
            }
        }
    }

    fn src_index(src: &mut InsecureSeededNoise, n: u64) -> u64 {
        use crate::synthesize::dp::noise::NoiseSource;
        src.next_u64() % n
    }

    #[test]
    fn noisy_counts_are_never_negative_and_stay_near_the_truth_at_a_large_epsilon() {
        let exact = vec![100, 0, 250, 0];
        let mut src = InsecureSeededNoise::new(1);
        let noisy = noisy_counts(&exact, 50.0, &mut src).unwrap();
        assert!(noisy.iter().all(|&c| c >= 0));
        for (n, e) in noisy.iter().zip(&exact) {
            assert!((n - e).abs() <= 5, "noisy {n} vs exact {e}");
        }
        let tiny = noisy_counts(&[0, 0, 0, 0, 0, 0], 0.01, &mut src).unwrap();
        assert!(
            tiny.iter().all(|&c| c >= 0),
            "negative noise on zero is clamped"
        );
    }

    #[test]
    fn invalid_domains_are_rejected() {
        assert!(Domain::Numeric {
            min: 5.0,
            max: 5.0,
            integer: false,
            bins: 4
        }
        .validate("x")
        .is_err());
        assert!(Domain::Numeric {
            min: 0.0,
            max: f64::INFINITY,
            integer: false,
            bins: 4
        }
        .validate("x")
        .is_err());
        assert!(Domain::Numeric {
            min: 0.0,
            max: 1.0,
            integer: false,
            bins: 1
        }
        .validate("x")
        .is_err());
        assert!(Domain::Categorical { values: vec![] }
            .validate("x")
            .is_err());
        assert!(Domain::Categorical {
            values: vec!["a".into(), "a".into()]
        }
        .validate("x")
        .is_err());
        assert!(Domain::Categorical {
            values: vec!["(other)".into()]
        }
        .validate("x")
        .is_err());
        assert!(numeric().validate("x").is_ok());
        assert!(
            Domain::Numeric {
                min: 0.0,
                max: 1.0,
                integer: false,
                bins: 2
            }
            .validate("x")
            .is_ok(),
            "two bins is the minimum"
        );
    }
}
