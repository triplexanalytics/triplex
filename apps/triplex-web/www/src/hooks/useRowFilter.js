import { useMemo, useState } from "react";
import { numericBinIndex } from "../state/binTooltip";

// Same fixed generalization width the Dataset Anonymity Analysis matrices
// score against (useDiversityMatrix.js's own RISK_BUCKET_WIDTH) -- a
// filter's bucket values only mean anything if resolved with the exact
// same width they were scored with.
const RISK_BUCKET_WIDTH = 10;

/** Row-level filter driven by clicking a matrix cell (DiversityCheckSection's
 * combined-k-anonymity matrix) or clicking/dragging a header-sparkline bar
 * span (HeaderSparkline, via binFilterEntry in binTooltip.js): narrows the
 * Table view to just the rows in one specific equivalence class -- an AND of
 * one or more conditions, one of:
 *   - { columnName, bucketValue } -- a matrix click's own equivalence-class
 *     bucket (RISK_BUCKET_WIDTH generalization for a numeric column, exact
 *     value otherwise).
 *   - { columnName, binIndexFrom, binIndexTo } -- a numeric sparkline span
 *     (inclusive); a plain click is binIndexFrom === binIndexTo.
 *   - { columnName, bucketValues } -- a categorical sparkline span, the set
 *     of exact values under the dragged bars (a plain click is one value).
 * A joint class needs every condition to match. `rowFilter` is that
 * condition list, `null` when no filter is active.
 *
 * Resolved synchronously on the main thread, not the datasetcard worker:
 * `wasm` is already loaded there via useWasm, and bucketing a couple of
 * quasi-identifier columns for a filter is cheap enough not to need a
 * worker round trip the way the O(rows x cols) full diversity matrix does. */
export function useRowFilter(wasm, sample, profile) {
  const [rowFilter, setRowFilter] = useState(null); // [{ columnName, ... }] | null

  const filteredRowIndices = useMemo(() => {
    if (!rowFilter || !wasm || !sample || !profile) return null;
    const colIndex = {};
    sample.headers.forEach((name, i) => (colIndex[name] = i));
    const resolvedPerFilter = rowFilter.map(({ columnName, binIndexFrom, bucketValues }) => {
      const col = profile.columns.find((c) => c.name === columnName);
      const idx = colIndex[columnName];
      if (idx === undefined) return [];
      if (binIndexFrom !== undefined) {
        // A sparkline bar's bin is display resolution (triplex-io's fixed
        // HISTOGRAM_BIN_COUNT), not a privacy bucket width -- resolve it
        // with numericBinIndex (the same math distribution.rs used to build
        // the bins in the first place), not the RISK_BUCKET_WIDTH scheme
        // below.
        const bins = col?.distribution?.bins;
        if (!bins?.length) return [];
        return Array.from(sample.getNumericColumn(idx), (v) => numericBinIndex(v, bins));
      }
      if (bucketValues !== undefined) {
        return Array.from({ length: sample.rowCount }, (_, row) => sample.getCell(row, idx));
      }
      return col?.inferred_type === "integer"
        ? Array.from(wasm.bucket_numeric_column(sample.getNumericColumn(idx), RISK_BUCKET_WIDTH))
        : Array.from({ length: sample.rowCount }, (_, row) => sample.getCell(row, idx));
    });
    const matches = new Set();
    for (let row = 0; row < sample.rowCount; row++) {
      const ok = rowFilter.every((f, fi) => {
        const value = resolvedPerFilter[fi][row];
        if (f.binIndexFrom !== undefined) return value >= f.binIndexFrom && value <= f.binIndexTo;
        if (f.bucketValues !== undefined) return f.bucketValues.includes(value);
        return value === f.bucketValue;
      });
      if (ok) matches.add(row);
    }
    return matches;
  }, [rowFilter, wasm, sample, profile]);

  return { rowFilter, setRowFilter, filteredRowIndices };
}
