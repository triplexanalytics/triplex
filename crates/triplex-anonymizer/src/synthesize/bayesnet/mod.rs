//! Tier 2 synthesis, slice T2.3: a discrete Bayesian network. Every column is
//! reduced to cells (numbers are binned, categories kept, the null is a value), a dependency
//! structure is learned, one conditional table is fitted per column given its parents, and rows
//! are drawn by sampling the columns parents-first.
//!
//! Unlike the Gaussian copula this keeps *categorical* dependencies, and with up to `k` parents it
//! keeps some multi-column structure. Two structures are offered ([`StructureKind`]): a Chow-Liu
//! tree and a k-parent network. A third kind, [`StructureKind::Independent`], is slice T2.4's
//! independent-marginals baseline: the same model with no parents, so every
//! dependency between columns is lost. [`synthesize_bayes`] makes no privacy claim; [`dp::synthesize_bayes_dp`]
//! is the differentially private variant (structure chosen by report-noisy-max, tables by noisy
//! counts).

pub mod dp;

mod cells;
mod model;
mod score;
mod structure;

pub use structure::{StructureKind, MAX_PARENTS};

use super::dp::Domain;
use super::{
    dropped_warnings, format_rows, metrics, prepare, round_to_observed, ColumnData, Preparation,
    SynthesisReport, SynthesisResult, SynthesizeConfig, OTHER_CATEGORY, PRIVACY_STATEMENT,
    REPORT_VERSION,
};
use cells::{Encoded, Table};
use model::Cpt;
use serde::Serialize;
use std::collections::BTreeSet;
use structure::{learn, Network, Selector};
use triplex_core::{Error, Result};
use triplex_io::Sample;

pub const DEFAULT_BINS: usize = 32;
/// A numeric column with at most this many distinct values is kept as categories: it can never
/// produce a value that did not occur in the data.
const SMALL_DISTINCT: usize = 20;

#[derive(Debug, Clone)]
pub struct BayesConfig {
    /// Rows, seed, rare-category threshold and the rows and columns the user removed.
    pub base: SynthesizeConfig,
    pub structure: StructureKind,
    /// Parents per column for [`StructureKind::KParent`] (1 to [`MAX_PARENTS`]); ignored for the tree.
    pub max_parents: usize,
    /// Equal-width bins per numeric column.
    pub bins: usize,
}

impl Default for BayesConfig {
    fn default() -> Self {
        BayesConfig {
            base: SynthesizeConfig::default(),
            structure: StructureKind::ChowLiu,
            max_parents: 2,
            bins: DEFAULT_BINS,
        }
    }
}

/// One node of the fitted network.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NodeReport {
    pub column: String,
    pub parents: Vec<String>,
    /// Cells in this node's conditional table (child cells x parent configurations).
    pub table_cells: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NetworkReport {
    pub structure: &'static str,
    pub max_parents: usize,
    pub bins: usize,
    /// Parents before children.
    pub nodes: Vec<NodeReport>,
}

/// The dependency structure as an HTML fragment (empty for a method without one). Shared by both
/// reports; every value is escaped.
pub(crate) fn network_html(network: Option<&NetworkReport>) -> String {
    let Some(network) = network else {
        return String::new();
    };
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let rows: String = network
        .nodes
        .iter()
        .map(|n| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&n.column),
                if n.parents.is_empty() {
                    "none (root)".to_string()
                } else {
                    esc(&n.parents.join(", "))
                },
                n.table_cells
            )
        })
        .collect();
    format!(
        "<h2>Dependency structure ({})</h2>\
         <table><thead><tr><th>Column</th><th>Conditioned on</th><th>Table cells</th></tr></thead><tbody>{rows}</tbody></table>",
        network.structure
    )
}

pub(crate) fn validate_shape(kind: StructureKind, max_parents: usize) -> Result<()> {
    if kind == StructureKind::KParent && !(1..=MAX_PARENTS).contains(&max_parents) {
        return Err(Error::Validation(format!(
            "max_parents must be between 1 and {MAX_PARENTS}"
        )));
    }
    Ok(())
}

pub(crate) fn network_report(
    names: &[String],
    enc: &Encoded,
    network: &Network,
    kind: StructureKind,
    max_parents: usize,
    bins: usize,
) -> NetworkReport {
    NetworkReport {
        structure: kind.as_str(),
        max_parents: match kind {
            StructureKind::ChowLiu => 1,
            StructureKind::KParent => max_parents,
            StructureKind::Independent => 0,
        },
        bins,
        nodes: network
            .order
            .iter()
            .map(|&i| NodeReport {
                column: names[i].clone(),
                parents: network.parents[i]
                    .iter()
                    .map(|&p| names[p].clone())
                    .collect(),
                table_cells: enc
                    .table_cells(i, &network.parents[i])
                    .map_or(0, |(_, total)| total),
            })
            .collect(),
    }
}

/// The domain and raw cell values of one prepared column.
fn column_domain(merged: &ColumnData, integer: bool, bins: usize) -> (Domain, Vec<Option<String>>) {
    match merged {
        ColumnData::Categorical(values) => {
            let distinct: BTreeSet<&str> = values
                .iter()
                .flatten()
                .map(String::as_str)
                .filter(|v| *v != OTHER_CATEGORY)
                .collect();
            (
                Domain::Categorical {
                    values: distinct.into_iter().map(str::to_string).collect(),
                },
                values.clone(),
            )
        }
        ColumnData::Numeric(values) => {
            let mut distinct: Vec<f64> = values.iter().flatten().copied().collect();
            distinct.sort_by(f64::total_cmp);
            distinct.dedup();
            let as_text = |x: f64| format!("{x}");
            let raw: Vec<Option<String>> = values.iter().map(|v| v.map(as_text)).collect();
            if distinct.len() <= SMALL_DISTINCT {
                return (
                    Domain::Categorical {
                        values: distinct.into_iter().map(as_text).collect(),
                    },
                    raw,
                );
            }
            let (min, max) = (distinct[0], distinct[distinct.len() - 1]);
            (
                Domain::Numeric {
                    min,
                    max,
                    integer,
                    bins,
                },
                raw,
            )
        }
    }
}

/// Turns sampled cells back into values: a bin becomes a uniform draw inside it, the null cell an
/// empty value.
pub(crate) fn cells_to_values(
    domains: &[Domain],
    cells: &[Vec<u32>],
    rng: &mut super::numeric::Rng,
) -> Vec<Vec<Option<String>>> {
    domains
        .iter()
        .zip(cells)
        .map(|(domain, column)| {
            column
                .iter()
                .map(|&cell| {
                    let cell = cell as usize;
                    (cell < domain.value_cells())
                        .then(|| super::dp::cell_value(domain, cell, rng.uniform()))
                })
                .collect()
        })
        .collect()
}

pub(crate) const INDEPENDENT_LIMITATION: &str = "Every column is drawn on its own: no dependency between columns is kept. This is a baseline to compare the other methods against, not a realistic table.";

pub(crate) fn limitations(kind: StructureKind) -> Vec<&'static str> {
    vec![
        if kind == StructureKind::Independent {
            INDEPENDENT_LIMITATION
        } else {
            "Dependencies are kept only between a column and its chosen parents; any dependency the network structure does not contain is lost."
        },
        "Numbers are binned (equal width), so the shape inside a bin is uniform and values are not reproduced exactly.",
        "Nulls are an ordinary value of each column, so missingness follows the same dependencies.",
        "Identifier-like columns and dates are dropped, not synthesized.",
    ]
}

/// Fits the network to `sample` and samples a synthetic table. See the module docs.
pub fn synthesize_bayes(sample: &Sample, config: &BayesConfig) -> Result<SynthesisResult> {
    validate_shape(config.structure, config.max_parents)?;
    if config.bins < 2 {
        return Err(Error::Validation("at least 2 bins are needed".into()));
    }
    let Preparation {
        prepared,
        dropped,
        merged_report,
        n,
        rows_excluded,
    } = prepare(sample, &config.base)?;

    let mut domains = Vec::new();
    let mut raw = Vec::new();
    for p in &prepared {
        let (domain, values) = column_domain(&p.merged, p.integer, config.bins);
        domains.push(domain);
        raw.push(values);
    }
    let enc = Encoded::new(domains, &raw);
    let names: Vec<String> = prepared.iter().map(|p| p.name.clone()).collect();

    let network = learn(
        &enc,
        config.structure,
        config.max_parents,
        &mut Selector::Exact,
    )?;
    let cpts: Vec<Cpt> = (0..enc.columns())
        .map(|i| {
            let table = Table::from_data(&enc, i, &network.parents[i]).ok_or_else(|| {
                Error::Validation(format!(
                    "column \"{}\" has too many distinct values for a Bayesian network",
                    names[i]
                ))
            })?;
            Ok(Cpt::from_counts(
                &table.counts,
                table.configs,
                table.child_arity,
            ))
        })
        .collect::<Result<_>>()?;

    let rows_out = config.base.rows.unwrap_or(n);
    let arities: Vec<usize> = (0..enc.columns()).map(|i| enc.arity(i)).collect();
    let mut rng = super::numeric::Rng::new(config.base.seed);
    let sampled = model::sample(&network, &cpts, &arities, rows_out, &mut rng);
    let values = cells_to_values(&enc.domains, &sampled, &mut rng);
    let mut synthetic: Vec<ColumnData> = values
        .into_iter()
        .zip(&prepared)
        .map(|(column, p)| match p.merged {
            ColumnData::Numeric(_) => ColumnData::Numeric(
                column
                    .into_iter()
                    .map(|v| v.and_then(|v| v.parse::<f64>().ok()))
                    .collect(),
            ),
            ColumnData::Categorical(_) => ColumnData::Categorical(column),
        })
        .collect();
    round_to_observed(&mut synthetic, &prepared);
    let rows = format_rows(&synthetic, &prepared, rows_out);

    let merged: Vec<ColumnData> = prepared.iter().map(|p| p.merged.clone()).collect();
    let originals: Vec<ColumnData> = prepared.iter().map(|p| p.original.clone()).collect();
    let report = SynthesisReport {
        version: REPORT_VERSION,
        method: match config.structure {
            StructureKind::ChowLiu => "bayesian_network_chow_liu",
            StructureKind::KParent => "bayesian_network_k_parent",
            StructureKind::Independent => "independent_marginals",
        },
        seed: config.base.seed,
        rows_in: n,
        rows_excluded,
        rows_out,
        columns_synthesized: names.clone(),
        columns_dropped: dropped.clone(),
        categories_merged: merged_report,
        correlation_shrinkage: 0.0,
        fidelity: metrics::fidelity(&names, &merged, &synthetic),
        leakage: metrics::leakage(&originals, &synthetic),
        privacy_statement: PRIVACY_STATEMENT,
        limitations: limitations(config.structure),
        warnings: dropped_warnings(&dropped),
        network: Some(network_report(
            &names,
            &enc,
            &network,
            config.structure,
            config.max_parents,
            config.bins,
        )),
    };
    Ok(SynthesisResult {
        sample: Sample {
            headers: names,
            rows,
        },
        report,
    })
}

/// Reads `input_path`, synthesizes, writes the CSV to `output_path` and returns the report.
pub fn synthesize_bayes_file(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    config: &BayesConfig,
) -> Result<SynthesisReport> {
    let sample = crate::read_rows(input_path)?;
    let result = synthesize_bayes(&sample, config)?;
    triplex_io::write_csv(output_path, &result.sample.headers, &result.sample.rows)?;
    Ok(result.report)
}

/// Byte-slice counterpart, for a caller with no filesystem (the browser).
pub fn synthesize_bayes_bytes(
    bytes: &[u8],
    extension: &str,
    config: &BayesConfig,
) -> Result<(Vec<u8>, SynthesisReport)> {
    let sample = triplex_schemacast::read_full_bytes(bytes, extension)?;
    let result = synthesize_bayes(&sample, config)?;
    let csv = triplex_io::write_csv_bytes(&result.sample.headers, &result.sample.rows)?;
    Ok((csv, result.report))
}
