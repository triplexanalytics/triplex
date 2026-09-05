//! Local I/O. No network calls. Reads customer data only from a local path or
//! from bytes the caller already holds in memory (e.g. a browser file
//! upload), and never transmits it anywhere.

mod formats;
mod shared;

pub use formats::arrow_ipc_sink::rows_to_arrow_ipc;
pub use formats::csv_sink::{write_csv, write_csv_bytes, write_csv_with_delimiter};
pub use formats::csv_source::{
    read_csv_full, read_csv_full_bytes, read_csv_full_with_delimiter, read_csv_sample,
    read_csv_sample_with_delimiter, CsvFormat,
};
pub use formats::format::{sample_format_for_extension, SampleFormat};
pub use formats::jsonl_sink::{write_jsonl, write_jsonl_bytes};
pub use formats::jsonl_source::{
    read_jsonl_full, read_jsonl_full_bytes, read_jsonl_sample, JsonlFormat,
};
pub use formats::parquet_sink::{write_parquet, WriteColumnType};
pub use formats::parquet_source::{read_parquet_rows, read_parquet_schema, ParquetColumnStats};
pub use formats::tsv_sink::{write_tsv, write_tsv_bytes};
pub use formats::tsv_source::{read_tsv_full, read_tsv_full_bytes, read_tsv_sample, TsvFormat};
pub use shared::distribution::{
    compute_column_distribution, CategoryCount, ColumnDistribution, HistogramBin, ValueKind,
};
pub use shared::sample::Sample;
pub use shared::statistics::{compute_column_statistics, ColumnStatistics, ColumnStatisticsBuilder};
