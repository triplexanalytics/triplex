/// The shape any ingestion format's bounded row sample takes once converted
/// to headers + string rows — independent of which file format produced it.
/// CSV, TSV, and JSONL each build one via their own reader; see
/// `SampleFormat` for how a format is chosen by file extension.
///
/// Row-major and fully owned (`max_rows × columns` individually heap-allocated
/// `String`s) — fine at today's fixed `sample_rows` default (~1000: bounded
/// and fast regardless of total file size, see the CLI's large-file test),
/// but this would need a streaming or column-major rework before either (a)
/// `max_rows` is exposed as a large/unbounded user-facing knob, or (b) a
/// full-file scan mode is added. Flagged for a performance pass before either
/// of those lands, not before.
pub struct Sample {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}
