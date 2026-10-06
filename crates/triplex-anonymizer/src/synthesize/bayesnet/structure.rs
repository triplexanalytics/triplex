//! Learning the dependency structure: which columns each column is conditioned on.
//!
//! * Chow-Liu tree: every column has at most one parent and the parents form a tree (Prim's
//!   algorithm on the pairwise scores, rooted at the first column).
//! * k-parent network (PrivBayes-style): columns are taken in order and each picks up to
//!   `max_parents` parents among the columns before it.
//!
//! Without a privacy guarantee each choice is the exact best score (mutual information). Under
//! differential privacy each choice is *report-noisy-max*: discrete Laplace noise on the integer
//! scaled [`r_score`], calibrated to twice the score sensitivity, then the arg-max. Which
//! candidates exist depends only on the public domains, never on the data, so the number of
//! private choices ([`planned_steps`]) is known before any budget is split.

use super::cells::{Encoded, Table};
use super::score::{mutual_information, r_score, r_sensitivity};
use crate::synthesize::dp::noise::add_noise;
use crate::synthesize::dp::{effective_epsilon, Accountant, NoiseSource};
use triplex_core::{Error, Result};

/// The most parents a column may have under [`StructureKind::KParent`].
pub const MAX_PARENTS: usize = 3;
/// More candidate parent sets than this for one column is refused (reduce the columns or `k`).
const MAX_CANDIDATES: usize = 2_000;
/// The score sensitivity, in integer score units, is `SCORE_UNITS + 1` (the `+ 1` covers the
/// rounding of the integer scaling).
const SCORE_UNITS: f64 = 1000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureKind {
    /// A tree: each column has at most one parent.
    ChowLiu,
    /// Each column has up to `max_parents` parents among the columns before it.
    KParent,
}

impl StructureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            StructureKind::ChowLiu => "chow_liu",
            StructureKind::KParent => "k_parent",
        }
    }
}

pub(crate) struct Network {
    /// Columns in an order where every parent comes before its child.
    pub(crate) order: Vec<usize>,
    pub(crate) parents: Vec<Vec<usize>>,
}

pub(crate) enum Selector<'a> {
    /// Best score, no privacy.
    Exact,
    /// Report-noisy-max, spending `epsilon_step` per choice.
    Private {
        noise: &'a mut dyn NoiseSource,
        accountant: &'a mut Accountant,
        epsilon_step: f64,
        n: usize,
    },
}

impl Selector<'_> {
    fn score(&self, table: &Table, n: usize) -> f64 {
        match self {
            Selector::Exact => mutual_information(table, n),
            Selector::Private { .. } => r_score(table, n),
        }
    }

    fn choose(&mut self, scores: &[f64], label: &str) -> Result<usize> {
        match self {
            Selector::Exact => Ok(argmax(scores.iter().copied())),
            Selector::Private {
                noise,
                accountant,
                epsilon_step,
                n,
            } => {
                let scale = SCORE_UNITS / r_sensitivity(*n);
                let sensitivity = SCORE_UNITS as u64 + 1;
                let used = effective_epsilon(*epsilon_step);
                // Report-noisy-max needs noise of scale 2 * sensitivity / epsilon for a score that
                // can move in either direction, i.e. the noise of an epsilon / 2 query.
                let noisy = scores
                    .iter()
                    .map(|&s| {
                        add_noise(
                            &mut **noise,
                            (s * scale).floor() as i64,
                            sensitivity,
                            used / 2.0,
                        )
                    })
                    .collect::<Result<Vec<i64>>>()?;
                accountant.spend(label, sensitivity, *epsilon_step, used)?;
                Ok(argmax(noisy.into_iter()))
            }
        }
    }
}

/// Index of the largest value; the first one wins a tie.
fn argmax<T: PartialOrd + Copy>(values: impl Iterator<Item = T>) -> usize {
    let mut best: Option<(usize, T)> = None;
    for (i, v) in values.enumerate() {
        if best.is_none_or(|(_, b)| v > b) {
            best = Some((i, v));
        }
    }
    best.map_or(0, |(i, _)| i)
}

fn binomial(m: usize, s: usize) -> usize {
    let s = s.min(m - s.min(m));
    let mut result = 1usize;
    for i in 0..s {
        result = match result.checked_mul(m - i) {
            Some(r) => r / (i + 1),
            None => return usize::MAX,
        };
    }
    result
}

/// All subsets of `0..m` with `size` elements, in lexicographic order.
fn combinations(m: usize, size: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut current = Vec::with_capacity(size);
    fn rec(
        start: usize,
        m: usize,
        size: usize,
        current: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == size {
            out.push(current.clone());
            return;
        }
        for i in start..m {
            current.push(i);
            rec(i + 1, m, size, current, out);
            current.pop();
        }
    }
    rec(0, m, size, &mut current, &mut out);
    out
}

/// Feasible candidate parent sets for column `i` under the k-parent rule: the largest size (at
/// most `max_parents`, at most `i`) that has any set whose table fits, else none. Depends only on
/// the public domains.
fn k_parent_candidates(enc: &Encoded, i: usize, max_parents: usize) -> Result<Vec<Vec<usize>>> {
    for size in (1..=max_parents.min(i)).rev() {
        if binomial(i, size) > MAX_CANDIDATES {
            return Err(Error::Validation(format!(
                "column {i} would have more than {MAX_CANDIDATES} candidate parent sets with up to {size} parents; use fewer columns or a smaller k"
            )));
        }
        let feasible: Vec<Vec<usize>> = combinations(i, size)
            .into_iter()
            .filter(|parents| enc.table_cells(i, parents).is_some())
            .collect();
        if !feasible.is_empty() {
            return Ok(feasible);
        }
    }
    Ok(Vec::new())
}

/// How many private choices [`learn`] will make. Public: it depends only on which conditional
/// tables fit, i.e. on the declared domains.
pub(crate) fn planned_steps(
    enc: &Encoded,
    kind: StructureKind,
    max_parents: usize,
) -> Result<usize> {
    let c = enc.columns();
    match kind {
        StructureKind::KParent => {
            let mut steps = 0;
            for i in 1..c {
                if !k_parent_candidates(enc, i, max_parents)?.is_empty() {
                    steps += 1;
                }
            }
            Ok(steps)
        }
        StructureKind::ChowLiu => {
            // Prim attaches every column reachable from a root, so the number of choices is
            // the column count minus the number of connected components of the "pair fits" graph.
            let mut component = vec![usize::MAX; c];
            let mut components = 0;
            for start in 0..c {
                if component[start] != usize::MAX {
                    continue;
                }
                component[start] = components;
                let mut stack = vec![start];
                while let Some(a) = stack.pop() {
                    #[allow(clippy::needless_range_loop)] // `b` also indexes the columns
                    for b in 0..c {
                        if component[b] == usize::MAX
                            && enc.table_cells(b, &[a]).is_some()
                            && enc.table_cells(a, &[b]).is_some()
                        {
                            component[b] = components;
                            stack.push(b);
                        }
                    }
                }
                components += 1;
            }
            Ok(c - components)
        }
    }
}

pub(crate) fn learn(
    enc: &Encoded,
    kind: StructureKind,
    max_parents: usize,
    selector: &mut Selector,
) -> Result<Network> {
    let c = enc.columns();
    let mut parents = vec![Vec::new(); c];
    let mut order = Vec::with_capacity(c);
    let n = enc.rows;
    match kind {
        StructureKind::KParent => {
            order.push(0);
            #[allow(clippy::needless_range_loop)] // `i` is a column index, not just a position
            for i in 1..c {
                let candidates = k_parent_candidates(enc, i, max_parents)?;
                if !candidates.is_empty() {
                    let scores: Vec<f64> = candidates
                        .iter()
                        .map(|p| {
                            let table =
                                Table::from_data(enc, i, p).expect("candidates are feasible");
                            selector.score(&table, n)
                        })
                        .collect();
                    let pick = selector.choose(&scores, &format!("structure:choice{i}"))?;
                    parents[i] = candidates[pick].clone();
                }
                order.push(i);
            }
        }
        StructureKind::ChowLiu => {
            let mut in_tree = vec![false; c];
            // Root the first tree at column 0; a column no tree can reach starts a new root.
            while order.len() < c {
                let mut candidates: Vec<(usize, usize)> = Vec::new(); // (child, parent)
                if !order.is_empty() {
                    for child in (0..c).filter(|&x| !in_tree[x]) {
                        for &parent in &order {
                            if enc.table_cells(child, &[parent]).is_some() {
                                candidates.push((child, parent));
                            }
                        }
                    }
                }
                if candidates.is_empty() {
                    let root = (0..c).find(|&x| !in_tree[x]).expect("a column is left");
                    in_tree[root] = true;
                    order.push(root);
                    continue;
                }
                let scores: Vec<f64> = candidates
                    .iter()
                    .map(|&(child, parent)| {
                        let table = Table::from_data(enc, child, &[parent]).expect("feasible");
                        selector.score(&table, n)
                    })
                    .collect();
                let pick = selector.choose(&scores, &format!("structure:choice{}", order.len()))?;
                let (child, parent) = candidates[pick];
                parents[child] = vec![parent];
                in_tree[child] = true;
                order.push(child);
            }
        }
    }
    Ok(Network { order, parents })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthesize::dp::{Domain, InsecureSeededNoise};

    fn categorical(values: usize) -> Domain {
        Domain::Categorical {
            values: (0..values).map(|v| format!("v{v}")).collect(),
        }
    }

    fn encoded(arities: &[usize], rows: &[Vec<usize>]) -> Encoded {
        // arity = declared values + the (other) cell + the null cell
        let domains: Vec<Domain> = arities.iter().map(|&a| categorical(a - 2)).collect();
        let raw: Vec<Vec<Option<String>>> = (0..arities.len())
            .map(|c| rows.iter().map(|r| Some(format!("v{}", r[c]))).collect())
            .collect();
        Encoded::new(domains, &raw)
    }

    #[test]
    fn choose_picks_the_best_score_exactly_and_the_first_on_a_tie() {
        assert_eq!(
            Selector::Exact.choose(&[0.1, 0.9, 0.9, 0.3], "x").unwrap(),
            1
        );
    }

    #[test]
    fn combinations_and_binomials_agree() {
        assert_eq!(combinations(5, 2).len(), binomial(5, 2));
        assert_eq!(combinations(4, 3).len(), 4);
        assert_eq!(combinations(3, 3), vec![vec![0, 1, 2]]);
        assert_eq!(binomial(30, 15), 155_117_520);
    }

    fn private_choice_counts(
        scores: &[f64],
        n: usize,
        epsilon: f64,
        trials: usize,
        seed: u64,
    ) -> Vec<usize> {
        let mut noise = InsecureSeededNoise::new(seed);
        let mut counts = vec![0; scores.len()];
        for _ in 0..trials {
            let mut accountant = Accountant::new(1e9).unwrap();
            let mut selector = Selector::Private {
                noise: &mut noise,
                accountant: &mut accountant,
                epsilon_step: epsilon,
                n,
            };
            counts[selector.choose(scores, "t").unwrap()] += 1;
        }
        counts
    }

    #[test]
    fn a_large_epsilon_finds_the_best_score_and_a_tiny_one_is_nearly_uniform() {
        let scores = [0.10, 0.45, 0.20];
        let sharp = private_choice_counts(&scores, 1000, 50.0, 300, 1);
        assert_eq!(sharp, vec![0, 300, 0]);
        let flat = private_choice_counts(&scores, 1000, 0.001, 3000, 2);
        for c in flat {
            assert!((900..1100).contains(&c), "not near-uniform: {c}");
        }
    }

    /// Report-noisy-max is epsilon-DP if, for scores that differ by at most the sensitivity
    /// (each moving either way), no option's selection probability changes by more than e^epsilon.
    /// Estimated by simulation with the worst-case shift, so it can only catch a gross error in
    /// the calibration, not prove the bound.
    #[test]
    fn selection_probabilities_stay_within_the_epsilon_bound_on_neighboring_scores() {
        let n = 100;
        let delta = r_sensitivity(n);
        let epsilon = 2.0;
        let base = [0.30, 0.28, 0.20];
        // every score moves by the full sensitivity, the best one down and the runner-up up
        let shifted = [base[0] - delta, base[1] + delta, base[2] + delta];
        let trials = 20_000;
        let a = private_choice_counts(&base, n, epsilon, trials, 11);
        let b = private_choice_counts(&shifted, n, epsilon, trials, 12);
        let bound = epsilon.exp() * 1.15; // slack for sampling error at these frequencies
        for i in 0..3 {
            let (pa, pb) = (a[i] as f64 / trials as f64, b[i] as f64 / trials as f64);
            if pa > 0.02 && pb > 0.02 {
                assert!(
                    pa / pb <= bound && pb / pa <= bound,
                    "option {i}: {pa} vs {pb}"
                );
            }
        }
        // ...and the shift does move probability, so the comparison is not trivially satisfied.
        assert!((a[0] as f64 - b[0] as f64).abs() > 100.0);
    }

    #[test]
    fn the_private_selector_spends_the_stated_budget_and_records_it() {
        let mut noise = InsecureSeededNoise::new(3);
        let mut accountant = Accountant::new(1.0).unwrap();
        {
            let mut selector = Selector::Private {
                noise: &mut noise,
                accountant: &mut accountant,
                epsilon_step: 0.25,
                n: 500,
            };
            selector.choose(&[0.1, 0.2], "structure:a").unwrap();
            selector.choose(&[0.3, 0.2], "structure:b").unwrap();
        }
        assert!((accountant.spent() - 0.5).abs() < 1e-9);
        assert_eq!(accountant.ledger().len(), 2);
        assert_eq!(accountant.ledger()[0].sensitivity, 1001);
    }

    #[test]
    fn planned_steps_matches_the_choices_actually_made() {
        // column 1 has so many values that no table with it fits, so it is isolated
        let arities = [4, 100_003, 5, 4];
        let rows: Vec<Vec<usize>> = (0..40).map(|r| vec![r % 2, r % 7, r % 3, r % 2]).collect();
        let enc = encoded(&arities, &rows);
        for (kind, k) in [
            (StructureKind::ChowLiu, 1),
            (StructureKind::KParent, 1),
            (StructureKind::KParent, 2),
            (StructureKind::KParent, 3),
        ] {
            let planned = planned_steps(&enc, kind, k).unwrap();
            let mut noise = InsecureSeededNoise::new(5);
            let mut accountant = Accountant::new(1e9).unwrap();
            let mut selector = Selector::Private {
                noise: &mut noise,
                accountant: &mut accountant,
                epsilon_step: 1.0,
                n: enc.rows,
            };
            let network = learn(&enc, kind, k, &mut selector).unwrap();
            assert_eq!(accountant.ledger().len(), planned, "{kind:?} k={k}");
            assert_eq!(network.order.len(), 4);
            assert!(
                network.parents[1].is_empty(),
                "the huge column cannot have parents"
            );
            for (i, p) in network.parents.iter().enumerate() {
                let position = |c: usize| network.order.iter().position(|&x| x == c).unwrap();
                assert!(
                    p.iter().all(|&q| position(q) < position(i)),
                    "parents come first"
                );
            }
        }
    }

    #[test]
    fn a_tree_has_at_most_one_parent_per_column_and_k_parent_respects_k() {
        let rows: Vec<Vec<usize>> = (0..200)
            .map(|r| vec![r % 3, r % 3, r % 5, (r / 2) % 4])
            .collect();
        let enc = encoded(&[5, 5, 7, 6], &rows);
        let tree = learn(&enc, StructureKind::ChowLiu, 3, &mut Selector::Exact).unwrap();
        assert!(tree.parents.iter().all(|p| p.len() <= 1));
        let kp = learn(&enc, StructureKind::KParent, 2, &mut Selector::Exact).unwrap();
        assert_eq!(kp.order, vec![0, 1, 2, 3]);
        assert!(kp.parents.iter().all(|p| p.len() <= 2));
        assert_eq!(kp.parents[3].len(), 2);
    }
}
