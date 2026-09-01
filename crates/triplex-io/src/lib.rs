//! Local I/O. No network calls. Reads customer data only from a local path or
//! from bytes the caller already holds in memory (e.g. a browser file
//! upload), and never transmits it anywhere.

mod csv_source;
mod delimited;
mod format;
mod jsonl_source;
mod parquet_sink;
mod parquet_source;
mod sample;
mod statistics;
mod tsv_source;

pub use csv_source::{
    read_csv_full, read_csv_full_bytes, read_csv_full_with_delimiter, read_csv_sample,
    read_csv_sample_with_delimiter, write_csv, write_csv_bytes, write_csv_with_delimiter,
    CsvFormat,
};
pub use format::{sample_format_for_extension, SampleFormat};
pub use jsonl_source::{
    read_jsonl_full, read_jsonl_full_bytes, read_jsonl_sample, write_jsonl, write_jsonl_bytes,
    JsonlFormat,
};
pub use parquet_sink::{write_parquet, WriteColumnType};
pub use parquet_source::{read_parquet_rows, read_parquet_schema, ParquetColumnStats};
pub use sample::Sample;
pub use statistics::{compute_column_statistics, ColumnStatistics, ColumnStatisticsBuilder};
pub use tsv_source::{
    read_tsv_full, read_tsv_full_bytes, read_tsv_sample, write_tsv, write_tsv_bytes, TsvFormat,
};
