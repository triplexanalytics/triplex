//! Data shapes shared across every format: the row `Sample` every source
//! produces and the `ColumnStatistics` every source can report.

pub(crate) mod distribution;
pub(crate) mod sample;
pub(crate) mod statistics;
