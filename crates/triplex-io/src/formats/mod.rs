//! One submodule per ingestion/output format, plus the `delimited` parser
//! shared by CSV/TSV and the `format` strategy trait that dispatches across
//! all of them by file extension.

pub(crate) mod arrow_ipc_sink;
pub(crate) mod arrow_ipc_source;
pub(crate) mod csv_sink;
pub(crate) mod csv_source;
pub(crate) mod delimited;
pub(crate) mod format;
pub(crate) mod jsonl_sink;
pub(crate) mod jsonl_source;
pub(crate) mod parquet_sink;
pub(crate) mod parquet_source;
pub(crate) mod tsv_sink;
pub(crate) mod tsv_source;
