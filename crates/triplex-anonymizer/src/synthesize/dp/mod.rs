//! Tier 2 slice T2.2: a differentially private Gaussian copula.
//!
//! The released parameters are noisy histograms (marginals) and noisy Kendall correlations, both
//! made with integer discrete-Laplace noise from a secure random source. Sampling from them is
//! post-processing and costs no privacy. The guarantee is pure epsilon-DP (delta = 0) for the
//! replace-one-record neighboring relation, treating the number of rows as public, and it assumes
//! the declared column list and bounds are public, i.e. not chosen by looking at this data.
//!
//! Nothing in [`DpReport`] is computed from the data except the noisy parameters' spend. The
//! fidelity and leakage figures of the non-private slice read the original data, so they are only
//! attached on explicit request, under a warning that the report must then stay private.

mod accountant;
pub(crate) mod histogram;
mod kendall;
pub(crate) mod noise;
mod report_html;
mod suggest;

pub use accountant::{Accountant, Spend};
pub use histogram::{Domain, OTHER_CATEGORY};
pub use noise::{effective_epsilon, InsecureSeededNoise, NoiseSource, OsNoise};
pub use report_html::render_html;
pub use suggest::{suggest_domains, SuggestedColumn, SuggestedDomain};

use super::copula::{repair_and_factor, ColumnData};
use super::metrics::{self, Fidelity, Leakage};
use super::numeric::{phi, Rng};
use serde::Serialize;
use triplex_core::{normalize_numeric_string, Error, Result};
use triplex_io::Sample;

pub const DP_REPORT_VERSION: &str = "0.1.0";
pub const DEFAULT_BINS: usize = 32;
pub const DEFAULT_MARGINAL_SHARE: f64 = 0.5;
/// Kendall's statistic is O(n^2) per column pair, so DP mode with two or more numeric columns
/// caps the row count.
pub const MAX_ROWS_WITH_CORRELATION: usize = 30_000;

pub const PRIVACY_STATEMENT: &str = "The fitted parameters (noisy histograms and noisy Kendall \
correlations) are epsilon-differentially private with respect to replacing one record, with the \
number of rows treated as public and the column list and bounds treated as public. Sampling from \
them is post-processing. Only this section carries the guarantee: anything computed from the \
original data is outside it.";

pub const DIAGNOSTICS_WARNING: &str = "These figures were computed from the original data and are \
NOT covered by the privacy guarantee. Do not publish this report, or share it with anyone who \
should not see the original data, alongside the synthetic file.";

#[derive(Debug, Clone)]
pub struct DpColumn {
    pub name: String,
    pub domain: Domain,
}

#[derive(Debug, Clone)]
pub struct DpConfig {
    /// The total privacy budget. Required: there is no default.
    pub epsilon: f64,
    /// Share of the budget spent on marginals; the rest goes to correlations.
    pub marginal_share: f64,
    pub columns: Vec<DpColumn>,
    /// Rows to generate; `None` means as many as the input (the row count is public).
    pub rows: Option<usize>,
    /// Seeds the sampling step only (post-processing). Noise never uses it.
    pub sampling_seed: u64,
    /// 0-based input rows the user removed; dropped before anything is counted. The row count the
    /// guarantee treats as public is the count after this exclusion.
    pub exclude_rows: std::collections::HashSet<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrivacySection {
    pub guarantee: &'static str,
    pub epsilon_total: f64,
    pub epsilon_spent: f64,
    pub delta: f64,
    pub neighbor_relation: &'static str,
    pub row_count_public: bool,
    pub bounds_and_columns_public: bool,
    pub marginal_mechanism: &'static str,
    pub correlation_mechanism: &'static str,
    pub noise_source: &'static str,
    pub epsilon_rounding: &'static str,
    pub statement: &'static str,
    pub ledger: Vec<Spend>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NonPrivateDiagnostics {
    pub warning: &'static str,
    pub fidelity: Fidelity,
    pub leakage: Leakage,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DpReport {
    pub version: &'static str,
    pub method: &'static str,
    pub sampling_seed: u64,
    /// Public by assumption.
    pub rows_in: usize,
    pub rows_excluded: usize,
    pub rows_out: usize,
    pub columns: Vec<String>,
    /// 0 when the noisy correlation matrix needed no repair.
    pub correlation_shrinkage: f64,
    pub privacy: PrivacySection,
    pub limitations: Vec<&'static str>,
    pub warnings: Vec<String>,
    /// Present only when explicitly requested; see [`NonPrivateDiagnostics`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonprivate_diagnostics: Option<NonPrivateDiagnostics>,
    /// The dependency structure, for the Bayesian-network method (public: it is part of the output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<super::bayesnet::NetworkReport>,
}

pub struct DpResult {
    pub sample: Sample,
    pub report: DpReport,
}

fn limitations() -> Vec<&'static str> {
    vec![
        "Only numeric column pairs are correlated; categorical columns are sampled independently of every other column.",
        "Kendall correlations over noisy counts are attenuated when ties or nulls are common, and noisy at small n or small epsilon.",
        "Histograms are equal-width over the declared bounds; the shape inside a bin is uniform.",
        "The number of rows, the column list and the bounds are assumed public.",
        "Nulls are modelled per column and independently of the other columns.",
    ]
}

fn build_warnings(shrinkage: f64, numeric_columns: usize, columns: usize) -> Vec<String> {
    let mut warnings = Vec::new();
    if shrinkage > 0.0 {
        warnings.push(format!("the noisy correlation matrix was not positive definite and was shrunk toward the identity by {shrinkage:.2}"));
    }
    if numeric_columns < columns {
        warnings.push(
            "categorical columns are sampled independently of every other column".to_string(),
        );
    }
    warnings
}

pub(crate) fn column_index(sample: &Sample, name: &str) -> Result<usize> {
    sample
        .headers
        .iter()
        .position(|h| h == name)
        .ok_or_else(|| Error::ColumnNotFound(name.to_string()))
}

pub(crate) fn cell(sample: &Sample, row: usize, column: usize) -> Option<&str> {
    sample.rows[row].get(column).map(String::as_str)
}

/// Numeric values clamped into the declared bounds; null and unparsable cells become `None`.
fn clamped_numbers(sample: &Sample, column: usize, min: f64, max: f64) -> Vec<Option<f64>> {
    (0..sample.rows.len())
        .map(|r| {
            let raw = cell(sample, r, column)?.trim();
            if raw.is_empty() {
                return None;
            }
            normalize_numeric_string(raw)
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite())
                .map(|x| x.clamp(min, max))
        })
        .collect()
}

struct Sampler {
    p_null: f64,
    cumulative: Vec<f64>,
    domain: Domain,
}

impl Sampler {
    fn from_noisy(domain: &Domain, noisy: &[i64]) -> Sampler {
        let cells = domain.value_cells();
        let value_total: i64 = noisy[..cells].iter().sum();
        let all_total: i64 = noisy.iter().sum();
        let p_null = if all_total > 0 {
            noisy[cells] as f64 / all_total as f64
        } else {
            0.0
        };
        let mut cumulative = Vec::with_capacity(cells);
        let mut running = 0.0;
        for (i, &count) in noisy[..cells].iter().enumerate() {
            // With no mass at all, fall back to uniform over the value cells (a public rule).
            let share = if value_total > 0 {
                count as f64 / value_total as f64
            } else {
                1.0 / cells as f64
            };
            running += share;
            cumulative.push(if i + 1 == cells { 1.0 } else { running });
        }
        Sampler {
            p_null,
            cumulative,
            domain: domain.clone(),
        }
    }

    fn draw(&self, u: f64, null_draw: f64) -> Option<String> {
        if null_draw < self.p_null {
            return None;
        }
        // A NaN position (never expected) is treated as the median rather than emitting NaN cells.
        let u = if u.is_nan() { 0.5 } else { u.clamp(0.0, 1.0) };
        let k = self
            .cumulative
            .iter()
            .position(|&c| u <= c)
            .expect("the last cumulative value is exactly 1 and u is at most 1");
        let lower = if k == 0 { 0.0 } else { self.cumulative[k - 1] };
        let width = self.cumulative[k] - lower;
        let fraction = if width > 0.0 {
            ((u - lower) / width).clamp(0.0, 1.0)
        } else {
            0.5
        };
        Some(cell_value(&self.domain, k, fraction))
    }
}

/// The value written for value-cell `k` of `domain`: a numeric bin becomes the point `fraction`
/// of the way through it, a category its label (`(other)` past the declared list).
pub(crate) fn cell_value(domain: &Domain, k: usize, fraction: f64) -> String {
    match domain {
        Domain::Numeric {
            min,
            max,
            integer,
            bins,
        } => {
            let bin_width = (max - min) / *bins as f64;
            let value = (min + (k as f64 + fraction) * bin_width).clamp(*min, *max);
            if *integer {
                format!("{}", value.round() as i64)
            } else {
                format!("{}", (value * 1e6).round() / 1e6)
            }
        }
        Domain::Categorical { values } => values
            .get(k)
            .cloned()
            .unwrap_or_else(|| OTHER_CATEGORY.to_string()),
    }
}

/// Kendall's statistic is quadratic, so two or more numeric columns cap the row count.
fn exceeds_row_cap(numeric_columns: usize, rows: usize) -> bool {
    numeric_columns >= 2 && rows > MAX_ROWS_WITH_CORRELATION
}

fn validate(sample: &Sample, config: &DpConfig) -> Result<()> {
    if !(config.epsilon.is_finite() && config.epsilon > 0.0) {
        return Err(Error::Validation(
            "a positive privacy budget (epsilon) is required; there is no default".into(),
        ));
    }
    if !(config.marginal_share > 0.0 && config.marginal_share < 1.0) {
        return Err(Error::Validation(
            "marginal_share must be strictly between 0 and 1".into(),
        ));
    }
    if config.columns.is_empty() {
        return Err(Error::Validation("declare at least one column with public bounds; DP mode does not choose columns from the data".into()));
    }
    let mut seen = std::collections::HashSet::new();
    for c in &config.columns {
        if !seen.insert(c.name.as_str()) {
            return Err(Error::Validation(format!(
                "column \"{}\" is declared twice",
                c.name
            )));
        }
        c.domain.validate(&c.name)?;
        column_index(sample, &c.name)?;
    }
    let numeric = config
        .columns
        .iter()
        .filter(|c| matches!(c.domain, Domain::Numeric { .. }))
        .count();
    if exceeds_row_cap(numeric, sample.rows.len()) {
        return Err(Error::Validation(format!(
            "DP mode with two or more numeric columns supports at most {MAX_ROWS_WITH_CORRELATION} rows (the correlation statistic is quadratic); got {}",
            sample.rows.len()
        )));
    }
    Ok(())
}

/// Fits the noisy model and samples from it. `noise` must be [`OsNoise`] for any real release.
pub fn synthesize_dp(
    sample: &Sample,
    config: &DpConfig,
    noise: &mut dyn NoiseSource,
) -> Result<DpResult> {
    let filtered;
    let (sample, rows_excluded) = if config.exclude_rows.is_empty() {
        (sample, 0)
    } else {
        let (kept, removed) = super::without_rows(sample, &config.exclude_rows);
        filtered = kept;
        (&filtered, removed)
    };
    validate(sample, config)?;
    let n = sample.rows.len();
    let k = config.columns.len();
    let indices: Vec<usize> = config
        .columns
        .iter()
        .map(|c| column_index(sample, &c.name))
        .collect::<Result<_>>()?;
    let numeric: Vec<usize> = (0..k)
        .filter(|&i| matches!(config.columns[i].domain, Domain::Numeric { .. }))
        .collect();
    let pairs = numeric.len() * numeric.len().saturating_sub(1) / 2;

    // Without numeric pairs there is nothing to correlate, so the marginals get the whole budget.
    let marginal_budget = if pairs == 0 {
        config.epsilon
    } else {
        config.epsilon * config.marginal_share
    };
    let per_column = marginal_budget / k as f64;
    let per_pair = if pairs == 0 {
        0.0
    } else {
        (config.epsilon - marginal_budget) / pairs as f64
    };

    let mut accountant = Accountant::new(config.epsilon)?;
    let mut samplers = Vec::with_capacity(k);
    for (c, &index) in config.columns.iter().zip(&indices) {
        let used = effective_epsilon(per_column);
        let exact = histogram::counts(
            &c.domain,
            (0..n).map(|r| cell(sample, r, index).map(str::to_string)),
        );
        let noisy = histogram::noisy_counts(&exact, used, noise)?;
        accountant.spend(
            &format!("histogram:{}", c.name),
            histogram::HISTOGRAM_SENSITIVITY,
            per_column,
            used,
        )?;
        samplers.push(Sampler::from_noisy(&c.domain, &noisy));
    }

    let mut correlation = vec![vec![0.0; k]; k];
    for (i, row) in correlation.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for (a, &i) in numeric.iter().enumerate() {
        for &j in &numeric[..a] {
            let bounds = |col: usize| match config.columns[col].domain {
                Domain::Numeric { min, max, .. } => (min, max),
                Domain::Categorical { .. } => unreachable!("numeric indices only"),
            };
            let (bi, bj) = (bounds(i), bounds(j));
            let x = clamped_numbers(sample, indices[i], bi.0, bi.1);
            let y = clamped_numbers(sample, indices[j], bj.0, bj.1);
            let used = effective_epsilon(per_pair);
            let rho = kendall::noisy_correlation(&x, &y, used, noise)?;
            accountant.spend(
                &format!(
                    "kendall:{}:{}",
                    config.columns[i].name, config.columns[j].name
                ),
                kendall::sensitivity(n),
                per_pair,
                used,
            )?;
            correlation[i][j] = rho;
            correlation[j][i] = rho;
        }
    }
    let (factor, shrinkage) = repair_and_factor(&correlation);

    let rows_out = config.rows.unwrap_or(n);
    let mut rng = Rng::new(config.sampling_seed);
    let rows: Vec<Vec<String>> = (0..rows_out)
        .map(|_| {
            let eps: Vec<f64> = (0..k).map(|_| rng.normal()).collect();
            (0..k)
                .map(|j| {
                    let z: f64 = (0..=j).map(|m| factor[j][m] * eps[m]).sum();
                    let null_draw = rng.uniform();
                    samplers[j].draw(phi(z), null_draw).unwrap_or_default()
                })
                .collect()
        })
        .collect();

    let warnings = build_warnings(shrinkage, numeric.len(), k);
    let names: Vec<String> = config.columns.iter().map(|c| c.name.clone()).collect();
    let report = DpReport {
        version: DP_REPORT_VERSION,
        method: "gaussian_copula_dp",
        sampling_seed: config.sampling_seed,
        rows_in: n,
        rows_excluded,
        rows_out,
        columns: names.clone(),
        correlation_shrinkage: shrinkage,
        privacy: PrivacySection {
            guarantee: "epsilon-differential privacy (delta = 0)",
            epsilon_total: config.epsilon,
            epsilon_spent: accountant.spent(),
            delta: 0.0,
            neighbor_relation: "replace one record",
            row_count_public: true,
            bounds_and_columns_public: true,
            marginal_mechanism: "discrete Laplace noise on histogram counts, L1 sensitivity 2",
            correlation_mechanism:
                "discrete Laplace noise on Kendall's C - D, L1 sensitivity 2(n - 1)",
            noise_source: "operating system secure random source",
            epsilon_rounding: "noise is calibrated to each epsilon rounded down to 6 decimals",
            statement: PRIVACY_STATEMENT,
            ledger: accountant.ledger().to_vec(),
        },
        limitations: limitations(),
        warnings,
        nonprivate_diagnostics: None,
        network: None,
    };
    Ok(DpResult {
        sample: Sample {
            headers: names,
            rows,
        },
        report,
    })
}

fn to_column_data(
    sample: &Sample,
    name: &str,
    domain: &Domain,
    map_unknown_to_other: bool,
) -> Result<ColumnData> {
    let index = column_index(sample, name)?;
    let raws = (0..sample.rows.len()).map(|r| {
        cell(sample, r, index)
            .map(str::trim)
            .filter(|v| !v.is_empty())
    });
    Ok(match domain {
        Domain::Numeric { .. } => ColumnData::Numeric(
            raws.map(|v| v.and_then(|v| normalize_numeric_string(v).parse::<f64>().ok()))
                .collect(),
        ),
        Domain::Categorical { values } => ColumnData::Categorical(
            raws.map(|v| {
                v.map(|v| {
                    if map_unknown_to_other && !values.iter().any(|d| d == v) {
                        OTHER_CATEGORY.to_string()
                    } else {
                        v.to_string()
                    }
                })
            })
            .collect(),
        ),
    })
}

/// Fidelity and leakage of `synthetic` against the original `real` table. **Computed from the
/// original data: not covered by the guarantee.** Attach with [`DpReport::with_diagnostics`].
pub fn nonprivate_diagnostics(
    real: &Sample,
    synthetic: &Sample,
    config: &DpConfig,
) -> Result<NonPrivateDiagnostics> {
    let mut mapped = Vec::new();
    let mut original = Vec::new();
    let mut synth = Vec::new();
    for c in &config.columns {
        mapped.push(to_column_data(real, &c.name, &c.domain, true)?);
        original.push(to_column_data(real, &c.name, &c.domain, false)?);
        synth.push(to_column_data(synthetic, &c.name, &c.domain, false)?);
    }
    let names: Vec<String> = config.columns.iter().map(|c| c.name.clone()).collect();
    Ok(NonPrivateDiagnostics {
        warning: DIAGNOSTICS_WARNING,
        fidelity: metrics::fidelity(&names, &mapped, &synth),
        leakage: metrics::leakage(&original, &synth),
    })
}

impl DpReport {
    pub fn with_diagnostics(mut self, diagnostics: NonPrivateDiagnostics) -> DpReport {
        self.nonprivate_diagnostics = Some(diagnostics);
        self
    }
}

/// Byte-slice counterpart to [`synthesize_dp_file`], for a caller with no filesystem (the
/// browser). Returns the synthetic table as CSV bytes and the report; with `with_diagnostics` the
/// report also carries the non-private diagnostics.
pub fn synthesize_dp_bytes(
    bytes: &[u8],
    extension: &str,
    config: &DpConfig,
    with_diagnostics: bool,
    noise: &mut dyn NoiseSource,
) -> Result<(Vec<u8>, DpReport)> {
    let real = triplex_schemacast::read_full_bytes(bytes, extension)?;
    let result = synthesize_dp(&real, config, noise)?;
    let csv = triplex_io::write_csv_bytes(&result.sample.headers, &result.sample.rows)?;
    let report = if with_diagnostics {
        let diagnostics = nonprivate_diagnostics(&real, &result.sample, config)?;
        result.report.with_diagnostics(diagnostics)
    } else {
        result.report
    };
    Ok((csv, report))
}

/// Builds a [`DpConfig`] from a `synthesize:` block. The budget is required (no default) and every
/// column must declare its public domain.
pub fn config_from_declared(
    declared: &triplex_schemacast::DeclaredSynthesize,
    rows: Option<usize>,
    sampling_seed: u64,
) -> Result<DpConfig> {
    use triplex_schemacast::DeclaredSynthesizeKind;
    let epsilon = declared.epsilon.ok_or_else(|| {
        Error::Validation(
            "the `synthesize:` block needs an `epsilon`; there is no default privacy budget".into(),
        )
    })?;
    let bins = declared.bins.unwrap_or(DEFAULT_BINS);
    let columns = declared
        .columns
        .iter()
        .map(|c| {
            let domain = match c.kind {
                DeclaredSynthesizeKind::Numeric => match (c.min, c.max) {
                    (Some(min), Some(max)) => Domain::Numeric { min, max, integer: c.integer, bins },
                    _ => {
                        return Err(Error::Validation(format!(
                            "numeric column \"{}\" must declare both `min` and `max` (public bounds, never taken from the data)",
                            c.name
                        )))
                    }
                },
                DeclaredSynthesizeKind::Categorical => Domain::Categorical { values: c.values.clone() },
            };
            Ok(DpColumn { name: c.name.clone(), domain })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DpConfig {
        epsilon,
        marginal_share: declared.marginal_share.unwrap_or(DEFAULT_MARGINAL_SHARE),
        columns,
        rows,
        sampling_seed,
        exclude_rows: std::collections::HashSet::new(),
    })
}

/// Reads `input_path`, synthesizes under the privacy budget, writes the CSV to `output_path` and
/// returns the report. With `with_diagnostics`, the report also carries the non-private
/// diagnostics (see [`NonPrivateDiagnostics`]).
pub fn synthesize_dp_file(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    config: &DpConfig,
    with_diagnostics: bool,
    noise: &mut dyn NoiseSource,
) -> Result<DpReport> {
    let real = crate::read_rows(input_path)?;
    let result = synthesize_dp(&real, config, noise)?;
    triplex_io::write_csv(output_path, &result.sample.headers, &result.sample.rows)?;
    if with_diagnostics {
        let diagnostics = nonprivate_diagnostics(&real, &result.sample, config)?;
        return Ok(result.report.with_diagnostics(diagnostics));
    }
    Ok(result.report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric_domain() -> Domain {
        Domain::Numeric {
            min: 0.0,
            max: 30.0,
            integer: false,
            bins: 3,
        }
    }

    #[test]
    fn the_report_lists_real_limitations() {
        let l = limitations();
        assert!(l.len() >= 4);
        assert!(l
            .iter()
            .any(|s| s.contains("categorical columns are sampled independently")));
        assert!(l.iter().any(|s| s.contains("public")));
        assert!(l.iter().all(|s| s.len() > 20));
    }

    #[test]
    fn the_row_cap_applies_from_two_numeric_columns_and_above_the_limit() {
        assert!(!exceeds_row_cap(2, MAX_ROWS_WITH_CORRELATION));
        assert!(exceeds_row_cap(2, MAX_ROWS_WITH_CORRELATION + 1));
        assert!(
            !exceeds_row_cap(1, MAX_ROWS_WITH_CORRELATION * 10),
            "one numeric column has no pairs"
        );
        assert!(!exceeds_row_cap(0, MAX_ROWS_WITH_CORRELATION * 10));
        assert!(exceeds_row_cap(3, MAX_ROWS_WITH_CORRELATION + 1));
    }

    #[test]
    fn a_sampler_over_noisy_counts_has_the_right_cumulative_shape_and_null_share() {
        // value cells 10, 30, 60; null cell 100 -> p_null 0.5
        let s = Sampler::from_noisy(&numeric_domain(), &[10, 30, 60, 100]);
        assert!((s.p_null - 0.5).abs() < 1e-12);
        assert!((s.cumulative[0] - 0.1).abs() < 1e-12 && (s.cumulative[1] - 0.4).abs() < 1e-12);
        assert_eq!(
            s.cumulative[2], 1.0,
            "the last cumulative value is exactly 1"
        );
    }

    #[test]
    fn the_last_cumulative_value_is_exactly_one_even_when_the_float_shares_do_not_sum_to_one() {
        // ten equal shares of 0.1 sum to 0.9999999999999999 in floating point; without forcing the
        // last value to 1.0 a draw at u = 1.0 would find no cell.
        let counts = vec![1i64; 10];
        let mut noisy = counts.clone();
        noisy.push(0);
        let domain = Domain::Numeric {
            min: 0.0,
            max: 10.0,
            integer: false,
            bins: 10,
        };
        let s = Sampler::from_noisy(&domain, &noisy);
        assert_eq!(*s.cumulative.last().unwrap(), 1.0);
        assert!(
            s.draw(1.0, 0.99).is_some(),
            "u = 1 must land in the last cell"
        );
    }

    #[test]
    fn with_no_mass_the_sampler_is_uniform_over_the_value_cells_and_never_null() {
        let s = Sampler::from_noisy(&numeric_domain(), &[0, 0, 0, 0]);
        assert_eq!(s.p_null, 0.0, "no mass at all: no null share, and no NaN");
        assert!(
            (s.cumulative[0] - 1.0 / 3.0).abs() < 1e-12
                && (s.cumulative[1] - 2.0 / 3.0).abs() < 1e-12
        );
        assert_eq!(s.cumulative[2], 1.0);
    }

    #[test]
    fn with_only_null_mass_the_values_fall_back_to_uniform() {
        let s = Sampler::from_noisy(&numeric_domain(), &[0, 0, 0, 5]);
        assert_eq!(s.p_null, 1.0);
        assert!(
            (s.cumulative[0] - 1.0 / 3.0).abs() < 1e-12,
            "uniform fallback when the value cells are empty"
        );
    }

    #[test]
    fn warnings_appear_only_when_they_are_true() {
        assert!(build_warnings(0.0, 3, 3).is_empty());
        let shrunk = build_warnings(0.02, 3, 3);
        assert_eq!(shrunk.len(), 1);
        assert!(shrunk[0].contains("shrunk toward the identity by 0.02"));
        let mixed = build_warnings(0.0, 2, 3);
        assert_eq!(mixed.len(), 1);
        assert!(mixed[0].contains("categorical columns are sampled independently"));
        assert_eq!(build_warnings(0.5, 0, 2).len(), 2);
    }

    #[test]
    fn the_null_draw_boundary_and_a_nan_position_are_handled() {
        let s = Sampler::from_noisy(&numeric_domain(), &[10, 10, 10, 30]);
        assert_eq!(s.p_null, 0.5);
        assert!(s.draw(0.5, 0.49).is_none(), "below p_null is null");
        assert!(s.draw(0.5, 0.5).is_some(), "at p_null is a value");
        let from_nan: f64 = s
            .draw(f64::NAN, 0.99)
            .unwrap()
            .parse()
            .expect("a number, never NaN");
        assert!(from_nan.is_finite() && (0.0..=30.0).contains(&from_nan));
        assert!(s.draw(7.0, 0.99).is_some() && s.draw(-3.0, 0.99).is_some());
    }

    #[test]
    fn a_zero_width_cell_yields_a_finite_in_range_value() {
        // the first cell has no mass, so u = 0 lands in a zero-width interval
        let s = Sampler::from_noisy(&numeric_domain(), &[0, 10, 10, 0]);
        let v: f64 = s
            .draw(0.0, 0.99)
            .unwrap()
            .parse()
            .expect("a finite number, not NaN");
        assert!((0.0..=30.0).contains(&v), "{v}");
    }

    #[test]
    fn categorical_draws_map_cells_to_declared_values_then_other() {
        let d = Domain::Categorical {
            values: vec!["a".into(), "b".into()],
        };
        let s = Sampler::from_noisy(&d, &[10, 10, 10, 0]);
        assert_eq!(s.draw(0.1, 0.99).as_deref(), Some("a"));
        assert_eq!(s.draw(0.5, 0.99).as_deref(), Some("b"));
        assert_eq!(s.draw(0.9, 0.99).as_deref(), Some(OTHER_CATEGORY));
    }
}
