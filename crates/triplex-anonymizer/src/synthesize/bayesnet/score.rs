//! Scores for "how much does a parent set tell about a column".
//!
//! * [`mutual_information`] is the classic Chow-Liu / PrivBayes score, used when no privacy
//!   guarantee is requested.
//! * [`r_score`] is the score the differentially private path uses. Its sensitivity is small and
//!   independent of the domain sizes, which mutual information's is not (see [`R_SENSITIVITY`]).

use super::cells::Table;

/// Mutual information (nats) between the child and its parent configuration.
pub(crate) fn mutual_information(table: &Table, n: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let n = n as f64;
    let mut child = vec![0i64; table.child_arity];
    let mut config = vec![0i64; table.configs];
    for (i, &c) in table.counts.iter().enumerate() {
        child[i % table.child_arity] += c;
        config[i / table.child_arity] += c;
    }
    let mut total = 0.0;
    for (i, &c) in table.counts.iter().enumerate() {
        if c > 0 {
            let joint = c as f64 / n;
            let expected = (child[i % table.child_arity] as f64 / n)
                * (config[i / table.child_arity] as f64 / n);
            total += joint * (joint / expected).ln();
        }
    }
    total.max(0.0)
}

/// `R = 1/2 * sum over every (config, child) cell of |p(child, config) - p(child) p(config)|`
/// (PrivBayes, Zhang et al. 2017): total-variation distance between the joint and the product of
/// its marginals. Zero-count cells are included, since they contribute `p(child) p(config)`.
pub(crate) fn r_score(table: &Table, n: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let n = n as f64;
    let mut child = vec![0i64; table.child_arity];
    let mut config = vec![0i64; table.configs];
    for (i, &c) in table.counts.iter().enumerate() {
        child[i % table.child_arity] += c;
        config[i / table.child_arity] += c;
    }
    let mut total = 0.0;
    for (i, &c) in table.counts.iter().enumerate() {
        let joint = c as f64 / n;
        let product =
            (child[i % table.child_arity] as f64 / n) * (config[i / table.child_arity] as f64 / n);
        total += (joint - product).abs();
    }
    total / 2.0
}

/// Upper bound on how much [`r_score`] changes when one record of `n` is replaced by another (the
/// row count is public and fixed). Derivation: replacing a record moves one unit of joint mass
/// between two cells, so `sum |d p(child, config)| <= 2/n`; the marginals each move by at most
/// `2/n` in L1, so `sum |d p(child) p(config)| <= 2/n + 2/n + 4/n^2`; hence
/// `|dR| <= (2/n + 2/n + 2/n + 4/n^2) / 2 = 3/n + 2/n^2`. A test checks this bound against
/// random neighboring tables.
pub(crate) fn r_sensitivity(n: usize) -> f64 {
    let n = n as f64;
    3.0 / n + 2.0 / (n * n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(child_arity: usize, configs: usize, counts: Vec<i64>) -> Table {
        Table {
            child_arity,
            configs,
            counts,
        }
    }

    #[test]
    fn independent_columns_score_zero() {
        // child uniform, config uniform, joint = product
        let t = table(2, 2, vec![25, 25, 25, 25]);
        assert!(mutual_information(&t, 100).abs() < 1e-12);
        assert!(r_score(&t, 100).abs() < 1e-12);
    }

    #[test]
    fn a_child_fully_determined_by_its_parent_scores_high() {
        let t = table(2, 2, vec![50, 0, 0, 50]);
        // I = ln 2 for two equally likely, perfectly linked values
        assert!((mutual_information(&t, 100) - std::f64::consts::LN_2).abs() < 1e-12);
        // R = 1/2 * (|.5-.25| + |0-.25| + |0-.25| + |.5-.25|) = 0.5
        assert!((r_score(&t, 100) - 0.5).abs() < 1e-12);
    }

    struct Xorshift(u64);
    impl Xorshift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    fn table_of(records: &[(usize, usize)], configs: usize, child_arity: usize) -> Table {
        let mut counts = vec![0i64; configs * child_arity];
        for &(config, child) in records {
            counts[config * child_arity + child] += 1;
        }
        table(child_arity, configs, counts)
    }

    /// The claim behind the differential-privacy guarantee of the structure choice: replacing one
    /// record never changes R by more than `r_sensitivity(n)`. Checked on many random small tables
    /// with every mix of domain sizes and record counts, including ones that put mass in a single
    /// cell.
    #[test]
    fn replacing_one_record_never_changes_r_by_more_than_the_stated_sensitivity() {
        let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
        let mut worst_ratio = 0.0f64;
        for trial in 0..4000 {
            let n = 2 + rng.below(60);
            let child_arity = 2 + rng.below(5);
            let configs = 1 + rng.below(6);
            let skew = trial % 3; // 0: uniform, 1: mass on few cells, 2: nearly one cell
            let draw = |rng: &mut Xorshift, limit: usize| match skew {
                0 => rng.below(limit),
                1 => rng.below(limit.min(2)),
                _ => usize::from(rng.below(10) == 0).min(limit - 1),
            };
            let records: Vec<(usize, usize)> = (0..n)
                .map(|_| (draw(&mut rng, configs), draw(&mut rng, child_arity)))
                .collect();
            let before = r_score(&table_of(&records, configs, child_arity), n);
            let mut changed = records.clone();
            let victim = rng.below(n);
            changed[victim] = (rng.below(configs), rng.below(child_arity));
            let after = r_score(&table_of(&changed, configs, child_arity), n);
            let bound = r_sensitivity(n);
            let delta = (after - before).abs();
            assert!(
                delta <= bound + 1e-12,
                "n={n} arity={child_arity} configs={configs}: |dR|={delta} > bound {bound}"
            );
            worst_ratio = worst_ratio.max(delta / bound);
        }
        // The bound is not vacuous: some neighboring pair gets a good part of the way to it.
        assert!(worst_ratio > 0.2, "worst observed ratio {worst_ratio}");
    }
}
