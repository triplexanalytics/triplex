//! Local file I/O. No network calls. Reads customer data only from a local path
//! the caller provides, and never transmits it anywhere.

mod csv_source;
mod parquet_source;
mod statistics;

pub use csv_source::{read_csv_full, read_csv_sample, write_csv, CsvSample};
pub use parquet_source::{read_parquet_rows, read_parquet_schema, ParquetColumnStats};
pub use statistics::{compute_column_statistics, ColumnStatistics, ColumnStatisticsBuilder};
