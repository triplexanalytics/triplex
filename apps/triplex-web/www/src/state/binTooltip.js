function formatNumber(n) {
  return Number.isInteger(n) ? String(n) : n.toFixed(2);
}

function plural(count) {
  return `${count} value${count === 1 ? "" : "s"}`;
}

/** One bar's description for a `ColumnDistribution` (see
 * `crates/triplex-io/src/shared/distribution.rs`) — shared by the Table
 * tab's header sparklines and the Column-view tab's standalone ones, so a
 * hovered bin reads the same everywhere the sparkline appears. */
export function binTooltip(distribution, index) {
  if (!distribution) return null;
  if (distribution.kind === "numeric") {
    const bin = distribution.bins[index];
    if (!bin) return null;
    return {
      title: `${formatNumber(bin.lower)} – ${formatNumber(bin.upper)}`,
      detail: plural(bin.count),
    };
  }
  const entry = distribution.top_values[index];
  if (!entry) return null;
  return { title: entry.value, detail: plural(entry.count) };
}

export function barCountFor(distribution) {
  if (!distribution) return 0;
  return distribution.kind === "numeric"
    ? distribution.bins.length
    : distribution.top_values.length;
}

export function barsFor(distribution) {
  if (!distribution) return [];
  return distribution.kind === "numeric"
    ? distribution.bins.map((b) => b.count)
    : distribution.top_values.map((v) => v.count);
}

/** A clicked bar's `useRowFilter` condition — a categorical bar's value is
 * already the exact cell value (same shape a matrix click's `bucketValue`
 * is), so it's just `{ columnName, bucketValue }`; a numeric bin has no
 * single value to match against (it's a range), so it's `{ columnName,
 * binIndex }` instead — see useRowFilter.js for how each is resolved back
 * to matching rows. */
export function binFilterEntry(columnName, distribution, index) {
  if (!distribution) return null;
  if (distribution.kind === "numeric") {
    return distribution.bins[index] ? { columnName, binIndex: index } : null;
  }
  const entry = distribution.top_values[index];
  return entry ? { columnName, bucketValue: entry.value } : null;
}
