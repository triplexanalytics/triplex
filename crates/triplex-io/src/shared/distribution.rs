use std::collections::HashMap;

use serde::Serialize;

/// Arbitrary implementation defaults, not a product decision — tune
/// freely if the browser table's sparklines need a different resolution.
const HISTOGRAM_BIN_COUNT: usize = 10;
const TOP_CATEGORY_LIMIT: usize = 12;

/// How a column's values should be summarized. Caller-supplied rather than
/// inferred here — `triplex-io` has no dependency on `triplex-schemacast`'s
/// `ColumnType`, and this keeps that one-directional layering unchanged.
///
/// Only two variants today because `ColumnType` itself only distinguishes
/// numeric/boolean/string. `ponytail:` this does not yet scale to column
/// *kinds* beyond numeric-vs-categorical (dates, IBAN, lat/long each want
/// their own binning — a date wants time-bucketed bins, a lat/long pair
/// wants a 2D/geo bucketing, not a 1D histogram or a flat value-count list).
/// Upgrade path once any of those exist as a real `ColumnType`: add a
/// variant here and a matching arm in `compute_column_distribution`, same
/// shape as this enum growing today — no rework of the numeric/categorical
/// paths required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Numeric,
    Categorical,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct HistogramBin {
    pub lower: f64,
    pub upper: f64,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CategoryCount {
    pub value: String,
    pub count: usize,
}

/// Batch, in-memory distribution summary for one column's sampled values —
/// the shape a Kaggle-style header sparkline or a column-detail panel needs,
/// beyond what the streaming `ColumnStatistics` gives. Kept separate from
/// `ColumnStatisticsBuilder`: that builder is deliberately streaming/no-`Vec`
/// for the CLI's large-file path, while quantiles need a sorted `Vec<f64>`
/// batch — only cheap because callers here already hold the full `Sample` in
/// memory. Format-agnostic like `ColumnStatistics`, so this is shared by the
/// CLI backend and any WASM/browser frontend built on `triplex-io`, not a
/// frontend-only computation.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColumnDistribution {
    Numeric {
        median: Option<f64>,
        std_dev: Option<f64>,
        p25: Option<f64>,
        p75: Option<f64>,
        bins: Vec<HistogramBin>,
    },
    Categorical {
        cardinality: usize,
        top_values: Vec<CategoryCount>,
    },
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = p * (sorted.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        return sorted[lower];
    }
    let frac = rank - lower as f64;
    sorted[lower] + (sorted[upper] - sorted[lower]) * frac
}

fn numeric_distribution(values: &[&str]) -> ColumnDistribution {
    let mut numbers: Vec<f64> = values
        .iter()
        .filter_map(|v| v.parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .collect();
    numbers.sort_by(|a, b| a.total_cmp(b));

    if numbers.len() < 2 {
        return ColumnDistribution::Numeric {
            median: None,
            std_dev: None,
            p25: None,
            p75: None,
            bins: Vec::new(),
        };
    }

    let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
    let variance = numbers.iter().map(|n| (n - mean).powi(2)).sum::<f64>() / numbers.len() as f64;

    let min = numbers[0];
    let max = numbers[numbers.len() - 1];
    let bins = if max > min {
        let width = (max - min) / HISTOGRAM_BIN_COUNT as f64;
        let mut counts = vec![0usize; HISTOGRAM_BIN_COUNT];
        for &n in &numbers {
            let idx = (((n - min) / width) as usize).min(HISTOGRAM_BIN_COUNT - 1);
            counts[idx] += 1;
        }
        counts
            .into_iter()
            .enumerate()
            .map(|(i, count)| HistogramBin {
                lower: min + width * i as f64,
                upper: min + width * (i + 1) as f64,
                count,
            })
            .collect()
    } else {
        vec![HistogramBin {
            lower: min,
            upper: max,
            count: numbers.len(),
        }]
    };

    ColumnDistribution::Numeric {
        median: Some(percentile(&numbers, 0.5)),
        std_dev: Some(variance.sqrt()),
        p25: Some(percentile(&numbers, 0.25)),
        p75: Some(percentile(&numbers, 0.75)),
        bins,
    }
}

/// A `HashMap` keyed by the borrowed `&str` (no per-value allocation until
/// the final top-N owns its strings) — O(n) counting. The previous version
/// used a linear `Vec` scan per value, O(n × distinct values): fine for a
/// handful of categories, but a high-cardinality column (e.g. free-text IDs
/// in a large file) made this quadratic.
fn categorical_distribution(values: &[&str]) -> ColumnDistribution {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for &v in values {
        if v.is_empty() {
            continue;
        }
        *counts.entry(v).or_insert(0) += 1;
    }
    let cardinality = counts.len();
    let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let top_values = ranked
        .into_iter()
        .take(TOP_CATEGORY_LIMIT)
        .map(|(value, count)| CategoryCount {
            value: value.to_string(),
            count,
        })
        .collect();

    ColumnDistribution::Categorical {
        cardinality,
        top_values,
    }
}

pub fn compute_column_distribution(values: &[&str], kind: ValueKind) -> ColumnDistribution {
    match kind {
        ValueKind::Numeric => numeric_distribution(values),
        ValueKind::Categorical => categorical_distribution(values),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_distribution_computes_median_stddev_and_quartiles() {
        let dist = compute_column_distribution(
            &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"],
            ValueKind::Numeric,
        );
        match dist {
            ColumnDistribution::Numeric {
                median,
                p25,
                p75,
                std_dev,
                bins,
            } => {
                assert_eq!(median, Some(5.5));
                assert_eq!(p25, Some(3.25));
                assert_eq!(p75, Some(7.75));
                assert!(std_dev.unwrap() > 0.0);
                assert_eq!(bins.len(), HISTOGRAM_BIN_COUNT);
                assert_eq!(bins.iter().map(|b| b.count).sum::<usize>(), 10);
            }
            other => panic!("expected Numeric, got {other:?}"),
        }
    }

    #[test]
    fn numeric_distribution_ignores_non_numeric_and_empty_cells() {
        let dist = compute_column_distribution(&["1", "", "not_a_number", "3"], ValueKind::Numeric);
        match dist {
            ColumnDistribution::Numeric { median, .. } => assert_eq!(median, Some(2.0)),
            other => panic!("expected Numeric, got {other:?}"),
        }
    }

    #[test]
    fn fewer_than_two_numeric_values_yields_none_stats_and_no_bins() {
        let dist = compute_column_distribution(&["1", "", "not_a_number"], ValueKind::Numeric);
        match dist {
            ColumnDistribution::Numeric {
                median,
                std_dev,
                p25,
                p75,
                bins,
            } => {
                assert_eq!(median, None);
                assert_eq!(std_dev, None);
                assert_eq!(p25, None);
                assert_eq!(p75, None);
                assert!(bins.is_empty());
            }
            other => panic!("expected Numeric, got {other:?}"),
        }
    }

    #[test]
    fn categorical_distribution_ranks_by_count_then_alphabetically() {
        let dist =
            compute_column_distribution(&["b", "a", "b", "c", "a", "b"], ValueKind::Categorical);
        match dist {
            ColumnDistribution::Categorical {
                cardinality,
                top_values,
            } => {
                assert_eq!(cardinality, 3);
                assert_eq!(
                    top_values,
                    vec![
                        CategoryCount {
                            value: "b".into(),
                            count: 3
                        },
                        CategoryCount {
                            value: "a".into(),
                            count: 2
                        },
                        CategoryCount {
                            value: "c".into(),
                            count: 1
                        },
                    ]
                );
            }
            other => panic!("expected Categorical, got {other:?}"),
        }
    }

    #[test]
    fn categorical_distribution_skips_empty_cells_and_caps_top_values() {
        let values: Vec<String> = (0..(TOP_CATEGORY_LIMIT + 5))
            .map(|i| format!("v{i}"))
            .collect();
        let mut refs: Vec<&str> = values.iter().map(String::as_str).collect();
        refs.push("");
        let dist = compute_column_distribution(&refs, ValueKind::Categorical);
        match dist {
            ColumnDistribution::Categorical {
                cardinality,
                top_values,
            } => {
                assert_eq!(cardinality, TOP_CATEGORY_LIMIT + 5);
                assert_eq!(top_values.len(), TOP_CATEGORY_LIMIT);
            }
            other => panic!("expected Categorical, got {other:?}"),
        }
    }

    #[test]
    fn categorical_distribution_treats_quotes_and_brackets_as_ordinary_characters() {
        // Values here are already-unquoted cell content (whatever a CSV/TSV/
        // JSONL reader handed back) — literal quote/bracket characters can
        // still appear inside that content, e.g. a name field or a
        // stringified list, and must count as plain text, not break parsing
        // or get merged with a similar-looking value.
        let dist = compute_column_distribution(
            &[
                "O'Brien",
                "O'Brien",
                "Say \"hi\"",
                "[1, 2, 3]",
                "{\"nested\": true}",
            ],
            ValueKind::Categorical,
        );
        match dist {
            ColumnDistribution::Categorical {
                cardinality,
                top_values,
            } => {
                assert_eq!(cardinality, 4);
                assert_eq!(
                    top_values[0],
                    CategoryCount {
                        value: "O'Brien".into(),
                        count: 2
                    }
                );
                assert!(top_values
                    .iter()
                    .any(|c| c.value == "Say \"hi\"" && c.count == 1));
                assert!(top_values
                    .iter()
                    .any(|c| c.value == "[1, 2, 3]" && c.count == 1));
                assert!(top_values
                    .iter()
                    .any(|c| c.value == "{\"nested\": true}" && c.count == 1));
            }
            other => panic!("expected Categorical, got {other:?}"),
        }
    }

    #[test]
    fn numeric_distribution_does_not_mistake_a_quoted_or_bracketed_string_for_a_number() {
        // A value like `"007"` or `[1]` fails `str::parse::<f64>`, same as
        // any other non-numeric string — it's silently excluded from the
        // numeric stats rather than causing a parse panic or corrupting the
        // mean/quantiles.
        let dist = compute_column_distribution(&["1", "\"007\"", "[1]", "3"], ValueKind::Numeric);
        match dist {
            ColumnDistribution::Numeric { median, .. } => assert_eq!(median, Some(2.0)),
            other => panic!("expected Numeric, got {other:?}"),
        }
    }
}
