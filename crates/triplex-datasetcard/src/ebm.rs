//! A from-scratch, Rust-native Explainable Boosting Machine (EBM; Lou et al.,
//! Microsoft Research) for per-*row* disclosure risk — an alternative
//! technique to `suda.rs`'s SUDA/DIS-SUDA, same shape (one score per row,
//! higher means riskier), switchable via `RowRiskModel` in `lib.rs`. See
//! the design notes and the research note this
//! promotes, the design notes.
//!
//! An EBM fits `g(E[y]) = β₀ + Σf_i(x_i) + Σf_ij(x_ij)`: one smooth "shape
//! function" per feature (plus, in the full published technique, automatic
//! pairwise-interaction terms), boosted *cyclically* — one feature at a
//! time, round-robin — specifically to limit co-linearity leakage between
//! features and keep every term independently plottable/auditable. This
//! module implements the `Σf_i(x_i)` main-effects term only (see "Deferred"
//! below) for `k` categorical quasi-identifier columns, where a "shape
//! function" is exactly what a GAM's shape function is for a categorical
//! feature: one learned contribution per distinct value, refit each round
//! against the residual of every other feature's current contribution.
//!
//! **Training target.** Unlike SUDA (which needs no training step — it's a
//! closed-form combinatorial search), an EBM needs something to predict.
//! This module trains against each row's *reciprocal joint equivalence-class
//! size* — `1.0 / (how many rows share this row's full combination of
//! values across all `k` columns)` — so a fully unique row's target is 1.0
//! (the ceiling) and a row sharing its combination with `n` others has
//! target `1/n`. This is the same "smaller class ⇒ riskier" direction
//! `k_anonymity` already scores on, just continuous instead of a bare count,
//! which is what lets an additive model learn a smooth per-value
//! contribution instead of only ever answering yes/no.
//!
//! **Where this runs.** the design notes left "where
//! would this run" as an open question, since the reference library
//! (InterpretML) is Python-only. Answered here: native Rust, trained from
//! scratch with no external ML dependency, so it runs in exactly the same
//! wasm/CLI/Python-binding surfaces every other DatasetCard metric already
//! does, with no Python runtime requirement.
//!
//! **Deferred** (ponytail: main-effects only, no pairwise interactions —
//! add `Σf_ij(x_ij)` cyclic boosting over column *pairs* if evaluation shows
//! single-column shape functions miss interaction-driven risk that SUDA's
//! exact combinatorial search already catches structurally; also deferred:
//! exposing the trained shape functions themselves as a queryable "why is
//! this risky" audit artifact — the research note's main motivation for
//! EBM over a black-box score — this module only returns the final summed
//! score today, not the per-feature decomposition, since no UI consumes a
//! decomposition yet).

use std::collections::HashMap;

use triplex_core::{Error, Result};

use crate::compose_quasi_identifiers;

/// Cyclic boosting rounds: one full pass reprocesses every column once.
/// Categorical shape functions converge fast (each column's update is
/// already a full per-category residual mean, not a small gradient step
/// restricted to one split), so a handful of rounds is enough for the
/// cross-column residual signal to settle — chosen empirically against this
/// module's own tests, not tuned against an external benchmark.
const EBM_ROUNDS: usize = 8;

/// Shrinkage applied to each round's per-category residual-mean update
/// before folding it into that column's shape function and the running
/// prediction. Below 1.0 so no single round's fit dominates and cyclic
/// boosting actually has rounds left to blend information across columns;
/// matches typical published EBM learning rates for main-effects terms.
const EBM_LEARNING_RATE: f64 = 0.3;

fn validate_columns(columns: &[Vec<String>]) -> Result<usize> {
    let rows = match columns.first() {
        Some(first) => first.len(),
        None => return Ok(0),
    };
    if columns.iter().any(|c| c.len() != rows) {
        return Err(Error::Validation(
            "ebm_scores: every quasi-identifier column must have the same number of rows"
                .to_string(),
        ));
    }
    Ok(rows)
}

/// Every row's EBM-predicted disclosure risk. `columns[j][i]` is
/// quasi-identifier column `j`'s value for row `i`; every column must have
/// the same length. Returns one score per row, same order and same
/// direction as `suda_scores` (0.0-ish means unremarkable, higher means
/// riskier) — not on the same numeric scale as SUDA's factorial weighting,
/// since this is a learned additive prediction rather than a combinatorial
/// count; see this module's doc comment for what it's trained against.
pub fn ebm_scores(columns: &[Vec<String>]) -> Result<Vec<f64>> {
    let k = columns.len();
    if k == 0 {
        return Ok(Vec::new());
    }
    let rows = validate_columns(columns)?;
    if rows == 0 {
        return Ok(Vec::new());
    }

    // Training target: reciprocal size of each row's full joint
    // equivalence class across all k columns together.
    let composite = compose_quasi_identifiers(columns);
    let mut class_counts: HashMap<&str, u32> = HashMap::new();
    for key in &composite {
        *class_counts.entry(key.as_str()).or_insert(0) += 1;
    }
    let target: Vec<f64> = composite
        .iter()
        .map(|key| 1.0 / f64::from(class_counts[key.as_str()]))
        .collect();

    let intercept = target.iter().sum::<f64>() / rows as f64;
    let mut prediction = vec![intercept; rows];
    let mut shape_functions: Vec<HashMap<String, f64>> = vec![HashMap::new(); k];

    for _round in 0..EBM_ROUNDS {
        for (j, column) in columns.iter().enumerate() {
            // This round's residual against every *other* term's current
            // contribution (this column's own prior contribution is still
            // baked into `prediction` here, which is correct: cyclic
            // boosting refits a feature's shape function against the
            // residual of the whole current model, including its own
            // earlier rounds, not a residual with itself subtracted out).
            let mut sum_by_value: HashMap<&str, f64> = HashMap::new();
            let mut count_by_value: HashMap<&str, u32> = HashMap::new();
            for i in 0..rows {
                let residual = target[i] - prediction[i];
                *sum_by_value.entry(column[i].as_str()).or_insert(0.0) += residual;
                *count_by_value.entry(column[i].as_str()).or_insert(0) += 1;
            }
            let mut update_by_value: HashMap<String, f64> = HashMap::new();
            for (value, sum) in &sum_by_value {
                let mean_residual = sum / f64::from(count_by_value[value]);
                update_by_value.insert((*value).to_string(), EBM_LEARNING_RATE * mean_residual);
            }
            for i in 0..rows {
                let update = update_by_value[column[i].as_str()];
                prediction[i] += update;
            }
            for (value, update) in update_by_value {
                *shape_functions[j].entry(value).or_insert(0.0) += update;
            }
        }
    }

    Ok(prediction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn no_columns_means_no_scores() {
        assert_eq!(ebm_scores(&[]).unwrap(), Vec::<f64>::new());
    }

    #[test]
    fn mismatched_column_lengths_is_a_clear_error() {
        let columns = vec![col(&["a", "b", "c"]), col(&["x", "y"])];
        let err = ebm_scores(&columns).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn no_rows_means_no_scores() {
        assert_eq!(ebm_scores(&[Vec::new()]).unwrap(), Vec::<f64>::new());
    }

    #[test]
    fn a_uniquely_combined_row_scores_higher_than_a_duplicated_one() {
        // Rows 0-2 all share (A=x, B=p); row 3 is the only (A=y, B=q).
        let a = col(&["x", "x", "x", "y"]);
        let b = col(&["p", "p", "p", "q"]);
        let scores = ebm_scores(&[a, b]).unwrap();
        assert_eq!(scores.len(), 4);
        assert!(
            scores[3] > scores[0],
            "unique row (score {}) should score higher than a triplicated row (score {})",
            scores[3],
            scores[0]
        );
        // The three duplicated rows are indistinguishable to the model —
        // same values on every column — so they must score identically.
        assert_eq!(scores[0], scores[1]);
        assert_eq!(scores[1], scores[2]);
    }

    #[test]
    fn every_row_unique_scores_uniformly_high() {
        let a = col(&["a1", "a2", "a3", "a4"]);
        let b = col(&["b1", "b2", "b3", "b4"]);
        let scores = ebm_scores(&[a, b]).unwrap();
        // Every row is its own equivalence class of size 1 (target 1.0
        // each) — a perfectly separable case the model should fit tightly,
        // landing close to the shared target rather than far from it.
        for score in scores {
            assert!(
                (score - 1.0).abs() < 0.3,
                "expected close to the fully-unique target of 1.0, got {score}"
            );
        }
    }

    #[test]
    fn deterministic_for_the_same_input() {
        let a = col(&["x", "x", "y", "z"]);
        let b = col(&["p", "q", "q", "r"]);
        let first = ebm_scores(&[a.clone(), b.clone()]).unwrap();
        let second = ebm_scores(&[a, b]).unwrap();
        assert_eq!(first, second);
    }

    /// Bounded-time check, same reasoning as `suda_scores`'s perf test —
    /// this technique is linear in rows/columns/rounds rather than
    /// exponential in columns, so it should comfortably clear a much larger
    /// case than SUDA's `MAX_QUASI_IDENTIFIER_COLUMNS`-bounded one.
    #[test]
    fn stays_fast_at_a_realistic_column_and_row_count() {
        use std::time::{Duration, Instant};

        let rows = 20_000;
        let columns: Vec<Vec<String>> = (0..8)
            .map(|c| (0..rows).map(|r| format!("c{c}v{}", r % 50)).collect())
            .collect();

        let start = Instant::now();
        let scores = ebm_scores(&columns).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(scores.len(), rows);
        assert!(
            elapsed < Duration::from_secs(5),
            "ebm_scores took {elapsed:?} for {rows} rows x 8 columns x {EBM_ROUNDS} rounds — expected well under 5s"
        );
    }
}
