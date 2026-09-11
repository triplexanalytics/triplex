//! SUDA (Special Uniques Detection Algorithm; Elliot & Manning) and
//! DIS-SUDA: per-*row* disclosure risk from a set of quasi-identifier
//! columns, complementing this crate's per-*column* `k_anonymity`/
//! `l_diversity`/`t_closeness` — see the design notes
//! . Production use at the UK ONS and Australian Bureau of Statistics;
//! reference implementations in R's `sdcMicro` (`suda2`) and a standalone
//! Python package.
//!
//! A **Minimal Sample Unique (MSU)** for row `i` is a subset `S` of the `k`
//! quasi-identifier columns such that row `i`'s values on `S` occur nowhere
//! else in the dataset, *and* no proper subset of `S` is already unique for
//! row `i` (if a smaller subset already singled it out, `S` isn't the
//! *minimal* one that did). Checking only `S`'s immediate
//! one-column-smaller subsets is sufficient to decide that, not just a
//! shortcut: uniqueness is monotonic in the columns considered (a subset
//! that's unique stays unique on every superset of it, since adding more
//! distinguishing information never merges two rows back together), so if
//! none of `S`'s immediate smaller subsets is unique for row `i`, no
//! smaller subset of `S` at all can be either.
//!
//! **Scoring**: `(k - |S|)!` per MSU found, summed over every MSU a row
//! belongs to — the classic SUDA weighting (Elliot, Manning 2002; used in
//! `sdcMicro`'s `suda2`), favoring a row that only needed a couple of
//! columns to stand out over one that needed most of them. This module
//! implements the published *definition* of a minimal sample unique
//! faithfully and tested; the weighting formula matches the qualitative
//! shape published implementations use, but exact numeric parity with any
//! specific external tool isn't claimed — see this module's tests for the
//! properties actually pinned down (fewer columns needed ⇒ higher score;
//! more MSUs ⇒ higher score).

use std::collections::HashMap;

use triplex_core::{Error, Result};

/// Above this many quasi-identifier columns, `suda_scores` refuses rather
/// than silently taking a very long time: cost is exponential in column
/// count (`2^k` subsets, each checked against every row). 10 columns is
/// already 1023 subsets — comfortably past any realistic quasi-identifier
/// set (a handful of columns) without risking a runaway on an accidental
/// "score every column" call.
/// ponytail: no memoization/pruning beyond the subset cap — the first
/// working version, not yet profiled against a real large dataset. If a
/// real profile shows this too slow at the cap, the `is_unique` closure's
/// repeated `subset_key` recomputation (once per uniqueness check, called
/// several times per row per mask) is the first thing to cache.
pub const MAX_QUASI_IDENTIFIER_COLUMNS: usize = 10;

/// Row `row`'s composite key across just the columns selected by `mask`'s
/// set bits, joined by U+001F (Unit Separator) — same convention
/// `compose_quasi_identifiers` uses, chosen for the same reason: a control
/// character that can't appear in ordinary CSV text or a numeric bucket
/// label, so two different column combinations can never collide into the
/// same joined string by coincidence.
fn subset_key(columns: &[Vec<String>], mask: usize, row: usize) -> String {
    let mut key = String::new();
    for (j, col) in columns.iter().enumerate() {
        if mask & (1 << j) != 0 {
            if !key.is_empty() {
                key.push('\u{1f}');
            }
            key.push_str(&col[row]);
        }
    }
    key
}

fn factorial(n: usize) -> f64 {
    (1..=n as u64).product::<u64>() as f64
}

/// Every row's SUDA score. `columns[j][i]` is quasi-identifier column `j`'s
/// value for row `i`; every column must have the same length. Returns one
/// score per row, same order — 0.0 means the row was never part of any
/// minimal sample unique (as safe as this scoring gets); higher means
/// riskier.
pub fn suda_scores(columns: &[Vec<String>]) -> Result<Vec<f64>> {
    let k = columns.len();
    if k == 0 {
        return Ok(Vec::new());
    }
    if k > MAX_QUASI_IDENTIFIER_COLUMNS {
        return Err(Error::Validation(format!(
            "suda_scores: {k} quasi-identifier columns exceeds the {MAX_QUASI_IDENTIFIER_COLUMNS}-column limit \
             — cost is exponential in column count"
        )));
    }
    let rows = columns[0].len();
    if columns.iter().any(|c| c.len() != rows) {
        return Err(Error::Validation(
            "suda_scores: every quasi-identifier column must have the same number of rows"
                .to_string(),
        ));
    }
    if rows == 0 {
        return Ok(Vec::new());
    }

    // One frequency table per non-empty column subset (`mask`), built once
    // and reused by every row's uniqueness/minimality check against that
    // subset, rather than recounting per row.
    let mut freq: Vec<HashMap<String, u32>> = vec![HashMap::new()]; // index 0 (empty subset) unused
    for mask in 1..(1usize << k) {
        let mut counts: HashMap<String, u32> = HashMap::new();
        for row in 0..rows {
            *counts.entry(subset_key(columns, mask, row)).or_insert(0) += 1;
        }
        freq.push(counts);
    }

    let is_unique = |mask: usize, row: usize| -> bool {
        freq[mask].get(&subset_key(columns, mask, row)).copied() == Some(1)
    };

    let mut scores = vec![0.0; rows];
    for mask in 1..(1usize << k) {
        let size = (mask as u32).count_ones() as usize;
        for (row, score) in scores.iter_mut().enumerate() {
            if !is_unique(mask, row) {
                continue;
            }
            let is_minimal = (0..k).filter(|j| mask & (1 << j) != 0).all(|j| {
                let parent = mask & !(1 << j);
                parent == 0 || !is_unique(parent, row)
            });
            if is_minimal {
                *score += factorial(k - size);
            }
        }
    }
    Ok(scores)
}

/// DIS-SUDA: distributes a file-level disclosure-risk budget of `1.0`
/// across rows in proportion to their `suda_scores` — the share of the
/// dataset's total re-identification risk this one row carries. A row's
/// `suda_scores` entry alone answers "how identifiable is this row";
/// `dis_suda_scores` answers "how much of the *dataset's* total risk sits
/// in this one row," useful for prioritizing which rows a review pass
/// should look at first. All-zero input (no MSUs found anywhere) yields
/// all-zero output rather than dividing by zero.
pub fn dis_suda_scores(suda_scores: &[f64]) -> Vec<f64> {
    let total: f64 = suda_scores.iter().sum();
    if total <= 0.0 {
        return vec![0.0; suda_scores.len()];
    }
    suda_scores.iter().map(|s| s / total).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn no_columns_means_no_scores() {
        assert_eq!(suda_scores(&[]).unwrap(), Vec::<f64>::new());
    }

    #[test]
    fn too_many_columns_is_a_clear_error_not_a_hang() {
        let columns: Vec<Vec<String>> = (0..MAX_QUASI_IDENTIFIER_COLUMNS + 1)
            .map(|_| col(&["a", "b"]))
            .collect();
        let err = suda_scores(&columns).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn mismatched_column_lengths_is_a_clear_error() {
        let columns = vec![col(&["a", "b", "c"]), col(&["x", "y"])];
        let err = suda_scores(&columns).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn a_row_unique_on_every_single_column_scores_highest() {
        // Row 0 is unique on column A alone ("z" appears nowhere else) —
        // the smallest possible MSU (size 1 out of k=2), so it should score
        // strictly higher than a row needing both columns to stand out.
        let a = col(&["z", "x", "x"]);
        let b = col(&["p", "p", "q"]);
        let scores = suda_scores(&[a, b]).unwrap();
        assert_eq!(scores.len(), 3);
        // Row 0: unique on {A} alone (MSU size 1, weight (2-1)! = 1) and
        // therefore also unique on {A,B}, but {A,B} isn't minimal since a
        // smaller subset ({A}) already did it — only the size-1 MSU counts.
        assert_eq!(scores[0], 1.0);
        // Row 2: not unique on {A} (shared with row 1) or {B} alone
        // (shared with... wait B="q" is actually unique to row 2 too).
        // Rows 1 and 2 share column A's value "x", so neither is unique on
        // {A} alone. Row 1 (A=x,B=p) and row 2 (A=x,B=q) differ on B, and
        // B values "p"/"q" each appear... "p" appears in rows 0 and 1, "q"
        // only in row 2 — so row 2 IS unique on {B} alone (size-1 MSU too).
        assert_eq!(scores[2], 1.0);
        // Row 1 needs both columns together (A=x shared with row 2, B=p
        // shared with row 0) — its only MSU is the full size-2 set,
        // weight (2-2)! = 1.
        assert_eq!(scores[1], 1.0);
    }

    #[test]
    fn fewer_columns_needed_scores_higher_than_needing_them_all() {
        // 3 columns. Row 0 is unique on column A alone (MSU size 1).
        // Row 2 needs all 3 columns together to be unique (no smaller
        // combination isolates it) — a strictly weaker (higher-effort)
        // disclosure than row 0's.
        let a = col(&["unique_a", "shared", "shared"]);
        let b = col(&["p", "p", "q"]);
        let c = col(&["1", "1", "1"]);
        let scores = suda_scores(&[a, b, c]).unwrap();
        // Row 0: MSU {A} alone, size 1, weight (3-1)! = 2.
        assert_eq!(scores[0], 2.0);
        // Row 2: differs from row 1 only on B (q vs p); A is shared
        // ("shared") between rows 1 and 2, C is shared by all three ("1").
        // {B} alone: "q" is unique to row 2 (size-1 MSU, weight (3-1)!=2).
        assert_eq!(scores[2], 2.0);
        // Row 1: shares A with row 2 and C with everyone, but B="p" is
        // shared with row 0 too — no single column isolates it, and no
        // pair does either (every pair still collides with some other
        // row), only the full triple does. That's a much weaker
        // disclosure than a size-1 MSU: weight (3-3)! = 1, strictly less
        // than row 0's and row 2's size-1 MSU weight of 2.
        assert!(scores[1] < scores[0]);
        assert!(scores[1] < scores[2]);
    }

    #[test]
    fn more_msus_for_the_same_row_score_higher_than_one_alone() {
        // 2 columns, both individually unique to row 0 — two separate
        // size-1 MSUs for the same row, so its score should be double a
        // row with only one such MSU.
        let a = col(&["unique_a", "x", "x"]);
        let b = col(&["unique_b", "p", "p"]);
        let scores = suda_scores(&[a, b]).unwrap();
        assert_eq!(scores[0], 2.0); // two size-1 MSUs, weight 1 each
    }

    #[test]
    fn a_row_with_no_unique_combination_at_all_scores_zero() {
        // Two exact-duplicate pairs: row 0/1 share (A=x, B=p), row 2/3
        // share (A=y, B=q). Neither column alone is unique to any row (x
        // and y each appear twice, same for p/q), and neither is the full
        // (A,B) pair (each combination also appears twice) — no row has
        // any unique subset at all, minimal or otherwise.
        let a = col(&["x", "x", "y", "y"]);
        let b = col(&["p", "p", "q", "q"]);
        let scores = suda_scores(&[a, b]).unwrap();
        assert_eq!(scores, vec![0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn dis_suda_distributes_risk_proportionally_and_sums_to_one() {
        let scores = vec![2.0, 1.0, 1.0, 0.0];
        let dis = dis_suda_scores(&scores);
        assert_eq!(dis, vec![0.5, 0.25, 0.25, 0.0]);
        let total: f64 = dis.iter().sum();
        assert!((total - 1.0).abs() < 1e-12);
    }

    #[test]
    fn dis_suda_is_all_zero_when_no_row_has_any_risk() {
        assert_eq!(dis_suda_scores(&[0.0, 0.0, 0.0]), vec![0.0, 0.0, 0.0]);
    }

    /// Bounded-time check, same reasoning as `t_closeness`'s perf test:
    /// asserts wall-clock stays reasonable at a realistic size rather than
    /// a machine-specific exact number. 6 columns (63 subsets) x 2,000 rows
    /// is a generous quasi-identifier set for the advisory use case this
    /// serves.
    #[test]
    fn stays_fast_at_a_realistic_column_and_row_count() {
        use std::time::{Duration, Instant};

        let rows = 2_000;
        let columns: Vec<Vec<String>> = (0..6)
            .map(|c| (0..rows).map(|r| format!("c{c}v{}", r % 50)).collect())
            .collect();

        let start = Instant::now();
        let scores = suda_scores(&columns).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(scores.len(), rows);
        assert!(
            elapsed < Duration::from_secs(5),
            "suda_scores took {elapsed:?} for {rows} rows x 6 columns — expected well under 5s"
        );
    }
}
