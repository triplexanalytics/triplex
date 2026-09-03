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
