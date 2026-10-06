//! The differentially private Bayesian network.
//!
//! Guarantee: epsilon-differential privacy (delta = 0) for replacing one record, with the row
//! count, the column list and each column's domain (bounds or category values) public. The budget
//! is split as follows and composed sequentially:
//!
//! * `structure_share` of epsilon goes to the dependency structure: one report-noisy-max choice
//!   per column that has a candidate parent set, each with an equal share. The score is
//!   [`r_score`](super::score::r_score) and its sensitivity is `3/n + 2/n^2` (see
//!   [`r_sensitivity`](super::score::r_sensitivity)).
//! * the rest goes to the conditional tables, one noisy table per column, each with an equal share
//!   (discrete Laplace noise on every cell, L1 sensitivity 2).
//!
//! With [`StructureKind::Independent`] (slice T2.4) there is no structure to
//! choose: `structure_share` is ignored, the whole budget goes to one noisy histogram per column,
//! and the ledger holds no structure entry.
//!
//! Sampling from the noisy tables is post-processing and costs nothing. Nothing else reads the
//! data. `noise` must be [`OsNoise`](crate::synthesize::dp::OsNoise) for any real release.

use super::cells::{Encoded, Table};
use super::model::{self, suppress_unsupported_cells, Cpt};
use super::structure::{learn, planned_steps, Selector, StructureKind};
use super::{cells_to_values, network_report, validate_shape};
use crate::synthesize::dp::histogram::{self, HISTOGRAM_SENSITIVITY};
use crate::synthesize::dp::{
    cell, column_index, effective_epsilon, nonprivate_diagnostics, Accountant, DpConfig, DpReport,
    DpResult, NoiseSource, PrivacySection, DP_REPORT_VERSION,
};
use crate::synthesize::numeric::Rng;
use std::collections::HashSet;
use triplex_core::{Error, Result};
use triplex_io::Sample;

pub const DEFAULT_STRUCTURE_SHARE: f64 = 0.3;

pub const BN_PRIVACY_STATEMENT: &str = "The dependency structure (chosen by report-noisy-max) and \
the conditional tables (noisy counts) are epsilon-differentially private with respect to replacing \
one record, assuming the row count, column list and column domains are public. Sampling from them \
is post-processing. Only this section carries the guarantee: anything computed from the original \
data is outside it.";

pub const MARGINALS_PRIVACY_STATEMENT: &str = "Each column's histogram (noisy counts) is epsilon-differentially private with respect to replacing one record, assuming the row count, column list and column domains are public. Columns are modelled independently, so nothing else is learned from the data. Sampling from the histograms is post-processing. Only this section carries the guarantee: anything computed from the original data is outside it.";

#[derive(Debug, Clone)]
pub struct BayesDpConfig {
    /// Budget, columns with their public domains, rows, sampling seed and removed rows. The
    /// `marginal_share` of the copula method is not used.
    pub dp: DpConfig,
    pub structure: StructureKind,
    /// Parents per column for [`StructureKind::KParent`]; ignored for the tree.
    pub max_parents: usize,
    /// Share of the budget spent choosing the structure (strictly between 0 and 1).
    pub structure_share: f64,
}

fn limitations(kind: StructureKind) -> Vec<&'static str> {
    let independent = kind == StructureKind::Independent;
    vec![
        if independent {
            super::INDEPENDENT_LIMITATION
        } else {
            "Dependencies are kept only between a column and its chosen parents; the structure is a noisy estimate and can pick a weaker edge than the best one."
        },
        if independent {
            "Each histogram has one cell per value: with many cells or a small budget most of it can be noise. A value whose total mass is indistinguishable from noise is dropped (post-processing), so rare values can disappear. See the warnings."
        } else {
            "Each table has one cell per (parent values, own value): with many cells or a small budget most of a table can be noise. A value whose total mass is indistinguishable from noise is dropped (post-processing), so rare values can disappear. See the warnings."
        },
        "Numbers are binned equal-width over the declared bounds; the shape inside a bin is uniform.",
        "The number of rows, the column list and the bounds are assumed public.",
        "Nulls are an ordinary value of each column.",
    ]
}

fn validate(sample: &Sample, config: &BayesDpConfig) -> Result<()> {
    let dp = &config.dp;
    if !(dp.epsilon.is_finite() && dp.epsilon > 0.0) {
        return Err(Error::Validation(
            "a positive privacy budget (epsilon) is required; there is no default".into(),
        ));
    }
    if !(config.structure_share > 0.0 && config.structure_share < 1.0) {
        return Err(Error::Validation(
            "structure_share must be strictly between 0 and 1".into(),
        ));
    }
    validate_shape(config.structure, config.max_parents)?;
    if dp.columns.is_empty() {
        return Err(Error::Validation("declare at least one column with public bounds; DP mode does not choose columns from the data".into()));
    }
    let mut seen = HashSet::new();
    for c in &dp.columns {
        if !seen.insert(c.name.as_str()) {
            return Err(Error::Validation(format!(
                "column \"{}\" is declared twice",
                c.name
            )));
        }
        c.domain.validate(&c.name)?;
        column_index(sample, &c.name)?;
    }
    if sample.rows.is_empty() {
        return Err(Error::Validation("the input has no rows".into()));
    }
    Ok(())
}

/// Fits the noisy network and samples from it.
pub fn synthesize_bayes_dp(
    sample: &Sample,
    config: &BayesDpConfig,
    noise: &mut dyn NoiseSource,
) -> Result<DpResult> {
    let filtered;
    let (sample, rows_excluded) = if config.dp.exclude_rows.is_empty() {
        (sample, 0)
    } else {
        let (kept, removed) = crate::synthesize::without_rows(sample, &config.dp.exclude_rows);
        filtered = kept;
        (&filtered, removed)
    };
    validate(sample, config)?;
    let n = sample.rows.len();
    let c = config.dp.columns.len();
    let names: Vec<String> = config.dp.columns.iter().map(|x| x.name.clone()).collect();
    let indices: Vec<usize> = names
        .iter()
        .map(|name| column_index(sample, name))
        .collect::<Result<_>>()?;
    let raw: Vec<Vec<Option<String>>> = indices
        .iter()
        .map(|&index| {
            (0..n)
                .map(|r| {
                    cell(sample, r, index)
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(str::to_string)
                })
                .collect()
        })
        .collect();
    let domains = config.dp.columns.iter().map(|x| x.domain.clone()).collect();
    let enc = Encoded::new(domains, &raw);

    // Public planning: how many private structure choices there will be, and the budget split.
    let steps = planned_steps(&enc, config.structure, config.max_parents)?;
    let structure_budget = if steps == 0 {
        0.0
    } else {
        config.dp.epsilon * config.structure_share
    };
    let per_step = structure_budget / steps.max(1) as f64;
    let per_table = (config.dp.epsilon - structure_budget) / c as f64;

    let mut accountant = Accountant::new(config.dp.epsilon)?;
    let network = {
        let mut selector = Selector::Private {
            noise: &mut *noise,
            accountant: &mut accountant,
            epsilon_step: per_step,
            n,
        };
        learn(&enc, config.structure, config.max_parents, &mut selector)?
    };

    let used_table = effective_epsilon(per_table);
    let independent = config.structure == StructureKind::Independent;
    let mut cpts = Vec::with_capacity(c);
    let mut warnings = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let table = Table::from_data(&enc, i, &network.parents[i]).ok_or_else(|| {
            Error::Validation(format!(
                "column \"{}\" has too many categories for a conditional table",
                name
            ))
        })?;
        let mut noisy = histogram::noisy_counts(&table.counts, used_table, noise)?;
        accountant.spend(
            &format!("table:{}", name),
            HISTOGRAM_SENSITIVITY,
            per_table,
            used_table,
        )?;
        let cells = table.counts.len();
        let per_cell = n as f64 / cells as f64;
        let noise_scale = HISTOGRAM_SENSITIVITY as f64 / used_table;
        suppress_unsupported_cells(&mut noisy, table.configs, table.child_arity, noise_scale);
        if per_cell < noise_scale {
            warnings.push(format!(
                "the table for \"{}\" has {cells} cells for {n} records (about {per_cell:.1} per cell) against noise of scale {noise_scale:.0} per cell; expect it to be mostly noise (use a larger epsilon{})",
                name,
                if independent { " or fewer bins" } else { ", fewer bins or fewer parents" }
            ));
        }
        cpts.push(Cpt::from_counts(&noisy, table.configs, table.child_arity));
    }

    let rows_out = config.dp.rows.unwrap_or(n);
    let arities: Vec<usize> = (0..c).map(|i| enc.arity(i)).collect();
    let mut rng = Rng::new(config.dp.sampling_seed);
    let sampled = model::sample(&network, &cpts, &arities, rows_out, &mut rng);
    let values = cells_to_values(&enc.domains, &sampled, &mut rng);
    let rows: Vec<Vec<String>> = (0..rows_out)
        .map(|r| {
            values
                .iter()
                .map(|column| column[r].clone().unwrap_or_default())
                .collect()
        })
        .collect();

    let report = DpReport {
        version: DP_REPORT_VERSION,
        method: match config.structure {
            StructureKind::ChowLiu => "bayesian_network_chow_liu_dp",
            StructureKind::KParent => "bayesian_network_k_parent_dp",
            StructureKind::Independent => "independent_marginals_dp",
        },
        sampling_seed: config.dp.sampling_seed,
        rows_in: n,
        rows_excluded,
        rows_out,
        columns: names.clone(),
        correlation_shrinkage: 0.0,
        privacy: PrivacySection {
            guarantee: "epsilon-differential privacy (delta = 0)",
            epsilon_total: config.dp.epsilon,
            epsilon_spent: accountant.spent(),
            delta: 0.0,
            neighbor_relation: "replace one record",
            row_count_public: true,
            bounds_and_columns_public: true,
            marginal_mechanism:
                "discrete Laplace noise on every cell of each column's table of counts given its parents, L1 sensitivity 2",
            correlation_mechanism: if independent {
                "none: columns are modelled independently, no dependency is estimated and no budget is spent on one"
            } else {
                "dependency structure chosen by report-noisy-max (discrete Laplace noise) on the total-variation score R, sensitivity 3/n + 2/n^2"
            },
            noise_source: "operating system secure random source",
            epsilon_rounding: "noise is calibrated to each epsilon rounded down to 6 decimals",
            statement: if independent {
                MARGINALS_PRIVACY_STATEMENT
            } else {
                BN_PRIVACY_STATEMENT
            },
            ledger: accountant.ledger().to_vec(),
        },
        limitations: limitations(config.structure),
        warnings,
        nonprivate_diagnostics: None,
        network: Some(network_report(
            &names,
            &enc,
            &network,
            config.structure,
            config.max_parents,
            config
                .dp
                .columns
                .iter()
                .find_map(|x| match x.domain {
                    crate::synthesize::dp::Domain::Numeric { bins, .. } => Some(bins),
                    _ => None,
                })
                .unwrap_or_default(),
        )),
    };
    Ok(DpResult {
        sample: Sample {
            headers: names,
            rows,
        },
        report,
    })
}

/// Byte-slice counterpart, for a caller with no filesystem (the browser). With
/// `with_diagnostics` the report also carries the non-private diagnostics (see
/// [`NonPrivateDiagnostics`](crate::synthesize::dp::NonPrivateDiagnostics)).
pub fn synthesize_bayes_dp_bytes(
    bytes: &[u8],
    extension: &str,
    config: &BayesDpConfig,
    with_diagnostics: bool,
    noise: &mut dyn NoiseSource,
) -> Result<(Vec<u8>, DpReport)> {
    let real = triplex_schemacast::read_full_bytes(bytes, extension)?;
    let result = synthesize_bayes_dp(&real, config, noise)?;
    let csv = triplex_io::write_csv_bytes(&result.sample.headers, &result.sample.rows)?;
    let report = if with_diagnostics {
        let diagnostics = nonprivate_diagnostics(&real, &result.sample, &config.dp)?;
        result.report.with_diagnostics(diagnostics)
    } else {
        result.report
    };
    Ok((csv, report))
}

/// True when a `synthesize:` block asks for a method this module runs: the Bayesian network, or
/// independent marginals (the same model with no parents).
pub fn is_bayes(declared: &triplex_schemacast::DeclaredSynthesize) -> bool {
    matches!(
        declared.method.as_deref(),
        Some("bayesian_network" | "independent_marginals")
    )
}

fn structure_from_name(name: Option<&str>) -> Result<StructureKind> {
    match name {
        None | Some("chow_liu") => Ok(StructureKind::ChowLiu),
        Some("k_parent") => Ok(StructureKind::KParent),
        Some(other) => Err(Error::Validation(format!(
            "structure must be \"chow_liu\" or \"k_parent\", got \"{other}\""
        ))),
    }
}

/// Builds a [`BayesDpConfig`] from a `synthesize:` block with `method: bayesian_network` or
/// `method: independent_marginals` (which takes no `structure`, `max_parents` or
/// `structure_share`). The budget and every column's public domain are required, as for the copula.
pub fn config_from_declared(
    declared: &triplex_schemacast::DeclaredSynthesize,
    rows: Option<usize>,
    sampling_seed: u64,
) -> Result<BayesDpConfig> {
    let structure = match declared.method.as_deref() {
        None | Some("bayesian_network") => structure_from_name(declared.structure.as_deref())?,
        Some("independent_marginals") => {
            if declared.structure.is_some()
                || declared.max_parents.is_some()
                || declared.structure_share.is_some()
            {
                return Err(Error::Validation(
                    "method \"independent_marginals\" has no structure: remove structure, max_parents and structure_share".into(),
                ));
            }
            StructureKind::Independent
        }
        Some(method) => {
            return Err(Error::Validation(format!(
                "method \"{method}\" is not a Bayesian network"
            )))
        }
    };
    Ok(BayesDpConfig {
        dp: crate::synthesize::dp::config_from_declared(declared, rows, sampling_seed)?,
        structure,
        max_parents: declared.max_parents.unwrap_or(2),
        structure_share: declared.structure_share.unwrap_or(DEFAULT_STRUCTURE_SHARE),
    })
}

/// Reads `input_path`, synthesizes under the privacy budget, writes the CSV and returns the report.
pub fn synthesize_bayes_dp_file(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    config: &BayesDpConfig,
    with_diagnostics: bool,
    noise: &mut dyn NoiseSource,
) -> Result<DpReport> {
    let real = crate::read_rows(input_path)?;
    let result = synthesize_bayes_dp(&real, config, noise)?;
    triplex_io::write_csv(output_path, &result.sample.headers, &result.sample.rows)?;
    if with_diagnostics {
        let diagnostics = nonprivate_diagnostics(&real, &result.sample, &config.dp)?;
        return Ok(result.report.with_diagnostics(diagnostics));
    }
    Ok(result.report)
}
