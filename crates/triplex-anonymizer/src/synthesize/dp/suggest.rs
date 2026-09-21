//! Suggested public domains for a UI to prefill a DP config from a file.
//!
//! **These are read from the data, so they are NOT public bounds.** DP mode requires bounds that
//! were declared without looking at this data (data-derived bounds leak). A UI may show these as a
//! convenience only if it makes the user confirm the values are public knowledge before a DP run
//!. Nothing here feeds a release by itself.

use serde::Serialize;
use std::collections::BTreeMap;
use triplex_core::normalize_numeric_string;
use triplex_io::Sample;
use triplex_schemacast::{infer_schema, ColumnType, SemanticType};

/// Categorical columns with more distinct values than this get no value list (a long list is not a
/// sensible public domain and is more likely an identifier).
pub const MAX_SUGGESTED_CATEGORIES: usize = 50;
const IDENTIFIER_UNIQUE_RATIO: f64 = 0.9;
const LOCATION_MAX_UNIQUE_RATIO: f64 = 0.5;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SuggestedDomain {
    Numeric {
        min: f64,
        max: f64,
        integer: bool,
    },
    Categorical {
        values: Vec<String>,
    },
    /// Nothing sensible to suggest (empty, or too many distinct values).
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SuggestedColumn {
    pub name: String,
    #[serde(flatten)]
    pub domain: SuggestedDomain,
    /// Whether a UI should pre-select the column. False for identifier-like columns.
    pub suggested_include: bool,
    /// Why a column is not pre-selected, or why it has no suggested domain.
    pub note: Option<String>,
    /// Always true: the values come from the data, so they are not public bounds.
    pub derived_from_data: bool,
}

/// Suggests a domain per column. Order follows the file's header order.
pub fn suggest_domains(sample: &Sample) -> Vec<SuggestedColumn> {
    let schema = infer_schema(sample);
    let n = sample.rows.len();
    schema
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let cells: Vec<&str> = (0..n)
                .filter_map(|r| sample.rows[r].get(index).map(|s| s.trim()))
                .filter(|v| !v.is_empty())
                .collect();
            let make =
                |domain: SuggestedDomain, include: bool, note: Option<&str>| SuggestedColumn {
                    name: column.name.clone(),
                    domain,
                    suggested_include: include,
                    note: note.map(str::to_string),
                    derived_from_data: true,
                };
            if cells.is_empty() {
                return make(SuggestedDomain::None, false, Some("every value is empty"));
            }
            let distinct: std::collections::HashSet<&str> = cells.iter().copied().collect();
            let unique_ratio = distinct.len() as f64 / cells.len() as f64;
            let identifier_like = match column.semantic_type {
                Some(SemanticType::Location) => unique_ratio >= LOCATION_MAX_UNIQUE_RATIO,
                Some(_) => true,
                None => {
                    column.inferred_type == ColumnType::String
                        && n >= 20
                        && unique_ratio >= IDENTIFIER_UNIQUE_RATIO
                }
            };
            if identifier_like {
                // Never list an identifier column's values as a suggested public domain.
                return make(
                    SuggestedDomain::None,
                    false,
                    Some("looks like an identifier"),
                );
            }
            let (include, note) = (true, None);
            match column.inferred_type {
                ColumnType::Integer | ColumnType::Float => {
                    let values: Vec<f64> = cells
                        .iter()
                        .filter_map(|v| normalize_numeric_string(v).parse::<f64>().ok())
                        .filter(|x| x.is_finite())
                        .collect();
                    let (min, max) = values
                        .iter()
                        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
                            (lo.min(x), hi.max(x))
                        });
                    if values.is_empty() || min >= max {
                        return make(
                            SuggestedDomain::None,
                            false,
                            Some("no numeric range to suggest"),
                        );
                    }
                    make(
                        SuggestedDomain::Numeric {
                            min,
                            max,
                            integer: column.inferred_type == ColumnType::Integer,
                        },
                        include,
                        note,
                    )
                }
                ColumnType::Boolean | ColumnType::String => {
                    if distinct.len() > MAX_SUGGESTED_CATEGORIES {
                        return make(
                            SuggestedDomain::None,
                            false,
                            Some("too many distinct values for a category list"),
                        );
                    }
                    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
                    for v in &cells {
                        *counts.entry(v).or_insert(0) += 1;
                    }
                    let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
                    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
                    make(
                        SuggestedDomain::Categorical {
                            values: ranked.into_iter().map(|(v, _)| v.to_string()).collect(),
                        },
                        include,
                        note,
                    )
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(headers: &[&str], rows: Vec<Vec<String>>) -> Sample {
        Sample {
            headers: headers.iter().map(|h| h.to_string()).collect(),
            rows,
        }
    }

    #[test]
    fn numeric_and_categorical_columns_get_a_domain_from_the_data() {
        let rows = (0..40)
            .map(|i| {
                vec![
                    (20 + i % 30).to_string(),
                    ["a", "b", "c"][i % 3].to_string(),
                    format!("{}.5", i % 7),
                ]
            })
            .collect();
        let s = suggest_domains(&table(&["age", "plan", "score"], rows));
        assert_eq!(
            s[0].domain,
            SuggestedDomain::Numeric {
                min: 20.0,
                max: 49.0,
                integer: true
            }
        );
        assert_eq!(
            s[1].domain,
            SuggestedDomain::Categorical {
                values: vec!["a".into(), "b".into(), "c".into()]
            }
        );
        assert_eq!(
            s[2].domain,
            SuggestedDomain::Numeric {
                min: 0.5,
                max: 6.5,
                integer: false
            }
        );
        assert!(s.iter().all(|c| c.suggested_include && c.derived_from_data));
    }

    #[test]
    fn categories_are_listed_most_frequent_first_then_alphabetically() {
        let mut rows: Vec<Vec<String>> = (0..6).map(|_| vec!["b".to_string()]).collect();
        rows.extend((0..3).map(|_| vec!["a".to_string()]));
        rows.extend((0..3).map(|_| vec!["c".to_string()]));
        let s = suggest_domains(&table(&["k"], rows));
        assert_eq!(
            s[0].domain,
            SuggestedDomain::Categorical {
                values: vec!["b".into(), "a".into(), "c".into()]
            }
        );
    }

    #[test]
    fn identifier_like_columns_are_not_preselected_get_no_domain_and_say_why() {
        let rows = (0..40)
            .map(|i| {
                vec![
                    format!("user{i}@example.com"),
                    format!("note number {i}"),
                    (i % 3).to_string(),
                ]
            })
            .collect();
        let s = suggest_domains(&table(&["email", "note", "tier"], rows));
        for c in &s[..2] {
            assert!(!c.suggested_include, "{}", c.name);
            assert_eq!(
                c.domain,
                SuggestedDomain::None,
                "an identifier's values must never be suggested: {}",
                c.name
            );
            assert_eq!(c.note.as_deref(), Some("looks like an identifier"));
        }
        assert!(s[2].suggested_include);
    }

    #[test]
    fn too_many_categories_and_empty_columns_have_no_suggested_domain() {
        let rows = (0..200)
            .map(|i| vec![format!("v{i}"), String::new()])
            .collect();
        let s = suggest_domains(&table(&["many", "empty"], rows));
        assert_eq!(s[0].domain, SuggestedDomain::None);
        assert_eq!(s[1].domain, SuggestedDomain::None);
        assert!(!s[0].suggested_include && !s[1].suggested_include);
    }

    #[test]
    fn a_constant_numeric_column_has_no_range_to_suggest() {
        let rows = (0..30).map(|_| vec!["5".to_string()]).collect();
        let s = suggest_domains(&table(&["c"], rows));
        assert_eq!(s[0].domain, SuggestedDomain::None);
    }

    #[test]
    fn every_suggestion_is_flagged_as_derived_from_the_data() {
        let rows = (0..30).map(|i| vec![(i % 5).to_string()]).collect();
        assert!(suggest_domains(&table(&["v"], rows))
            .iter()
            .all(|c| c.derived_from_data));
    }

    #[test]
    fn a_suggestion_serializes_with_a_flat_type_tag() {
        let rows = (0..30).map(|i| vec![(i % 5).to_string()]).collect();
        let json = serde_json::to_value(&suggest_domains(&table(&["v"], rows))[0]).unwrap();
        assert_eq!(json["type"], "numeric");
        assert_eq!(json["name"], "v");
        assert_eq!(json["derived_from_data"], true);
    }
}
