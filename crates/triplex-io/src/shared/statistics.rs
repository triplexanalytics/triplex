use serde::Serialize;

/// Format-agnostic per-column statistics: how much of a column is missing,
/// and its numeric range/mean where that's meaningful. Shared by every
/// ingestion format so schemacast's job is just interpreting these plus
/// inferring a type, not recomputing them per format.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ColumnStatistics {
    pub null_percentage: f64,
    /// `None` for a column with no parseable numbers at all (e.g. a genuine
    /// string or boolean column).
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    /// `None` wherever a true mean isn't cheaply available — e.g. Parquet's
    /// footer statistics carry min/max but no sum, and computing one would
    /// mean decoding row data, which breaks that path's zero-copy design.
    pub mean_value: Option<f64>,
}

/// Incrementally accumulates the null/min/max/mean statistics that make up
/// `ColumnStatistics`, one batch at a time, with no intermediate `Vec`
/// allocation — a prior version collected every parsed number into a `Vec<f64>`
/// per column before reducing it, which is wasted allocation on every read.
/// Both the CSV path (`record_value` per cell) and the Parquet path
/// (`record_batch` per row group's already-aggregated stats) build on this,
/// so the min/max-merge and null-percentage math exist in exactly one place.
#[derive(Debug, Default)]
pub struct ColumnStatisticsBuilder {
    total_count: u64,
    null_count: u64,
    min_value: Option<f64>,
    max_value: Option<f64>,
    sum: f64,
    numeric_count: u64,
}

impl ColumnStatisticsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one raw cell: empty counts as null; otherwise it counts toward
    /// the numeric range/mean only if it parses as a finite number.
    pub fn record_value(&mut self, value: &str) {
        self.total_count += 1;
        if value.is_empty() {
            self.null_count += 1;
            return;
        }
        if let Ok(n) = triplex_core::normalize_numeric_string(value).parse::<f64>() {
            if n.is_finite() {
                self.record_number(n);
            }
        }
    }

    fn record_number(&mut self, n: f64) {
        self.min_value = Some(self.min_value.map_or(n, |m: f64| m.min(n)));
        self.max_value = Some(self.max_value.map_or(n, |m: f64| m.max(n)));
        self.sum += n;
        self.numeric_count += 1;
    }

    /// Merges in a batch's null/total counts and numeric min/max directly —
    /// for sources like Parquet row groups that already carry aggregated
    /// statistics rather than raw values. Never contributes to the mean
    /// (no sum is available at this granularity), matching `ColumnStatistics`'s
    /// documented `mean_value` behavior for such sources.
    pub fn record_batch(
        &mut self,
        null_count: u64,
        total_count: u64,
        min: Option<f64>,
        max: Option<f64>,
    ) {
        self.null_count += null_count;
        self.total_count += total_count;
        if let Some(min) = min {
            self.min_value = Some(self.min_value.map_or(min, |m: f64| m.min(min)));
        }
        if let Some(max) = max {
            self.max_value = Some(self.max_value.map_or(max, |m: f64| m.max(max)));
        }
    }

    pub fn build(self) -> ColumnStatistics {
        let null_percentage = if self.total_count == 0 {
            0.0
        } else {
            self.null_count as f64 / self.total_count as f64 * 100.0
        };
        let mean_value = if self.numeric_count == 0 {
            None
        } else {
            Some(self.sum / self.numeric_count as f64)
        };
        ColumnStatistics {
            null_percentage,
            min_value: self.min_value,
            max_value: self.max_value,
            mean_value,
        }
    }
}

/// Computes null/min/max/mean statistics for one column's sampled string
/// values in a single pass.
pub fn compute_column_statistics(values: &[&str]) -> ColumnStatistics {
    let mut builder = ColumnStatisticsBuilder::new();
    for value in values {
        builder.record_value(value);
    }
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_naive_definition_on_a_small_mixed_column() {
        let stats = compute_column_statistics(&["1", "", "3", "5.5", "not_a_number"]);
        assert_eq!(stats.null_percentage, 20.0); // 1 of 5 is empty
        assert_eq!(stats.min_value, Some(1.0));
        assert_eq!(stats.max_value, Some(5.5));
        assert_eq!(stats.mean_value, Some((1.0 + 3.0 + 5.5) / 3.0));
    }

    #[test]
    fn record_batch_never_contributes_to_the_mean() {
        let mut builder = ColumnStatisticsBuilder::new();
        builder.record_batch(1, 4, Some(1.0), Some(9.0));
        let stats = builder.build();
        assert_eq!(stats.null_percentage, 25.0);
        assert_eq!(stats.min_value, Some(1.0));
        assert_eq!(stats.max_value, Some(9.0));
        assert_eq!(stats.mean_value, None);
    }

    #[test]
    fn record_batch_can_be_merged_across_multiple_batches() {
        let mut builder = ColumnStatisticsBuilder::new();
        builder.record_batch(0, 2, Some(5.0), Some(10.0));
        builder.record_batch(1, 2, Some(1.0), Some(20.0));
        let stats = builder.build();
        assert_eq!(stats.null_percentage, 25.0); // 1 null of 4 total
        assert_eq!(stats.min_value, Some(1.0));
        assert_eq!(stats.max_value, Some(20.0));
    }
}
