import { useMemo, useState } from "react";

// Same fixed generalization width the Dataset Anonymity Analysis matrices
// score against (useDiversityMatrix.js's own RISK_BUCKET_WIDTH) -- a
// filter's bucket values only mean anything if resolved with the exact
// same width they were scored with.
const RISK_BUCKET_WIDTH = 10;

/** Row-level filter driven by clicking a matrix cell (DiversityCheckSection's
 * combined-k-anonymity matrix) or a header-sparkline bar (HeaderSparkline,
 * via binFilterEntry in binTooltip.js): narrows the Table view to just the
 * rows in one specific equivalence class -- an AND of one or more
 * { columnName, bucketValue } or { columnName, binIndex } conditions (a
 * joint class needs both columns' buckets to match). `rowFilter` is that
 * condition list, `null` when no filter is active.
 *
 * Resolved synchronously on the main thread, not the datasetcard worker:
 * `wasm` is already loaded there via useWasm, and bucketing a couple of
 * quasi-identifier columns for a filter is cheap enough not to need a
 * worker round trip the way the O(rows x cols) full diversity matrix does. */
export function useRowFilter(wasm, sample, profile) {
  const [rowFilter, setRowFilter] = useState(null); // [{ columnName, bucketValue }] | null

  const filteredRowIndices = useMemo(() => {
    if (!rowFilter || !wasm || !sample || !profile) return null;
    const colIndex = {};
    sample.headers.forEach((name, i) => (colIndex[name] = i));
    const bucketedPerFilter = rowFilter.map(({ columnName, binIndex }) => {
      const col = profile.columns.find((c) => c.name === columnName);
      const idx = colIndex[columnName];
      if (idx === undefined) return [];
      if (binIndex !== undefined) {
        // A sparkline bar's bin is display resolution (triplex-io's fixed
        // HISTOGRAM_BIN_COUNT), not a privacy bucket width -- resolve it
        // with the same min/width/clamp math distribution.rs used to build
        // the bins in the first place, not the RISK_BUCKET_WIDTH scheme
        // below.
        const bins = col?.distribution?.bins;
        if (!bins?.length) return [];
        const min = bins[0].lower;
        const width = bins[0].upper - bins[0].lower;
        return Array.from(sample.getNumericColumn(idx), (v) =>
          Number.isFinite(v) ? Math.min(Math.floor((v - min) / width), bins.length - 1) : -1,
        );
      }
      return col?.inferred_type === "integer"
        ? Array.from(wasm.bucket_numeric_column(sample.getNumericColumn(idx), RISK_BUCKET_WIDTH))
        : Array.from({ length: sample.rowCount }, (_, row) => sample.getCell(row, idx));
    });
    const matches = new Set();
    for (let row = 0; row < sample.rowCount; row++) {
      if (
        rowFilter.every(
          (f, fi) => bucketedPerFilter[fi][row] === (f.binIndex !== undefined ? f.binIndex : f.bucketValue),
        )
      ) {
        matches.add(row);
      }
    }
    return matches;
  }, [rowFilter, wasm, sample, profile]);

  return { rowFilter, setRowFilter, filteredRowIndices };
}
