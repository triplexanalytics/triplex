import { useMemo } from "react";
import { numericBinIndex } from "../state/binTooltip";

/** Per-column bar counts restricted to `filteredRowIndices` (useRowFilter.js),
 * aligned bar-for-bar with each column's own `col.distribution` (same bins/
 * top_values order and length) -- coordinated-view brushing: HeaderSparkline
 * overlays this on top of its own grayed-out full-dataset baseline once a
 * row filter is active anywhere in the app, not just on the column that
 * originated it (a matrix click or another column's sparkline drag counts
 * too). `{ [columnName]: number[] }`, or `null` when no filter is active,
 * so every card just renders its normal single-layer histogram.
 *
 * ponytail: one synchronous main-thread pass per column, same cost model
 * useRowFilter.js's own binIndexFrom resolution already accepts (including
 * `sample.getNumericColumn`'s own O(rows) rebuild per numeric column) --
 * move to the datasetcard worker if this measurably lags on a very wide or
 * many-row file. */
export function useColumnSubsetDistributions(sample, profile, filteredRowIndices) {
  return useMemo(() => {
    if (!filteredRowIndices || !sample || !profile) return null;
    const colIndex = {};
    sample.headers.forEach((name, i) => (colIndex[name] = i));

    const result = {};
    for (const col of profile.columns) {
      const idx = colIndex[col.name];
      const distribution = col.distribution;
      if (idx === undefined || !distribution) continue;

      if (distribution.kind === "numeric") {
        const bins = distribution.bins;
        const counts = new Array(bins.length).fill(0);
        const values = sample.getNumericColumn(idx);
        for (const row of filteredRowIndices) {
          const bin = numericBinIndex(values[row], bins);
          if (bin >= 0) counts[bin] += 1;
        }
        result[col.name] = counts;
      } else {
        const topValues = distribution.top_values;
        const indexByValue = new Map(topValues.map((v, i) => [v.value, i]));
        const counts = new Array(topValues.length).fill(0);
        for (const row of filteredRowIndices) {
          const bin = indexByValue.get(sample.getCell(row, idx));
          if (bin !== undefined) counts[bin] += 1;
        }
        result[col.name] = counts;
      }
    }
    return result;
  }, [sample, profile, filteredRowIndices]);
}
