//! Every column as a small discrete variable: a record maps to exactly one *cell* of each column's
//! domain through a public function (the same one the DP histograms use, `Domain::cell`), so the
//! model is a Bayesian network over cells. The null cell is an ordinary value of the variable.

use super::super::dp::Domain;

/// Largest conditional table (child cells x parent-configuration cells) a node may have. Tables
/// beyond this are neither scored nor fitted: they would be mostly empty (and, under DP, mostly
/// noise). The rule depends only on the public domains, never on the data.
pub(crate) const MAX_TABLE_CELLS: usize = 100_000;

pub(crate) struct Encoded {
    pub(crate) domains: Vec<Domain>,
    /// `cells[column][row]`: the cell index of that record in that column (the null cell is
    /// `domains[column].value_cells()`).
    pub(crate) cells: Vec<Vec<u32>>,
    pub(crate) rows: usize,
}

impl Encoded {
    /// Maps raw values (already trimmed; `None` for an empty cell) into cells.
    pub(crate) fn new(domains: Vec<Domain>, raw: &[Vec<Option<String>>]) -> Encoded {
        let rows = raw.first().map_or(0, Vec::len);
        let cells = domains
            .iter()
            .zip(raw)
            .map(|(domain, column)| {
                column
                    .iter()
                    .map(|v| domain.cell(v.as_deref()) as u32)
                    .collect()
            })
            .collect();
        Encoded {
            domains,
            cells,
            rows,
        }
    }

    pub(crate) fn columns(&self) -> usize {
        self.domains.len()
    }

    /// Number of values of a column, the null cell included.
    pub(crate) fn arity(&self, column: usize) -> usize {
        self.domains[column].value_cells() + 1
    }

    /// Parent configurations of a parent set (their product of arities), or `None` when that
    /// overflows or the conditional table for `child` would exceed [`MAX_TABLE_CELLS`].
    pub(crate) fn table_cells(&self, child: usize, parents: &[usize]) -> Option<(usize, usize)> {
        let mut configs = 1usize;
        for &p in parents {
            configs = configs.checked_mul(self.arity(p))?;
        }
        let total = configs.checked_mul(self.arity(child))?;
        (total <= MAX_TABLE_CELLS).then_some((configs, total))
    }

    /// The index of a record's parent configuration (mixed radix over the parents in order).
    pub(crate) fn config_of(&self, row: usize, parents: &[usize]) -> usize {
        parents.iter().fold(0, |acc, &p| {
            acc * self.arity(p) + self.cells[p][row] as usize
        })
    }
}

/// Counts over (parent configuration, child cell): `counts[config * child_arity + child_cell]`.
pub(crate) struct Table {
    pub(crate) child_arity: usize,
    pub(crate) configs: usize,
    pub(crate) counts: Vec<i64>,
}

impl Table {
    pub(crate) fn from_data(enc: &Encoded, child: usize, parents: &[usize]) -> Option<Table> {
        let (configs, total) = enc.table_cells(child, parents)?;
        let child_arity = enc.arity(child);
        let mut counts = vec![0i64; total];
        for row in 0..enc.rows {
            counts[enc.config_of(row, parents) * child_arity + enc.cells[child][row] as usize] += 1;
        }
        Some(Table {
            child_arity,
            configs,
            counts,
        })
    }
}
