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

/** Which of a numeric column's own `distribution.bins` a raw value falls
 * into — the exact min/width/clamp math `crates/triplex-io/src/shared/
 * distribution.rs`'s `numeric_distribution` used to build those bins in the
 * first place (uniform width, last bin also catches the max value). Shared
 * by useRowFilter.js's binIndexFrom range-matching and
 * useColumnSubsetDistributions.js's per-column recount so the two can't
 * silently drift apart the way two independent reimplementations of "which
 * bin is this" would. `-1` for a non-finite value or an empty `bins`. */
export function numericBinIndex(value, bins) {
  if (!bins?.length || !Number.isFinite(value)) return -1;
  const min = bins[0].lower;
  const width = bins[0].upper - bins[0].lower;
  return Math.min(Math.floor((value - min) / width), bins.length - 1);
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

/** A clicked-or-dragged bar span's `useRowFilter` condition, from `fromIndex`
 * to `toIndex` inclusive (either order; a plain click omits `toIndex`, so a
 * single bar is `fromIndex === toIndex`). A numeric span has no single value
 * to match against (it's a range of bins), so it's `{ columnName,
 * binIndexFrom, binIndexTo }`; a categorical span is a set of exact cell
 * values (a single-bar click is a one-element set), so it's `{ columnName,
 * bucketValues }` — see useRowFilter.js for how each is resolved back to
 * matching rows. */
export function binFilterEntry(columnName, distribution, fromIndex, toIndex = fromIndex) {
  if (!distribution) return null;
  const lo = Math.min(fromIndex, toIndex);
  const hi = Math.max(fromIndex, toIndex);
  if (distribution.kind === "numeric") {
    if (!distribution.bins[lo] || !distribution.bins[hi]) return null;
    return { columnName, binIndexFrom: lo, binIndexTo: hi };
  }
  const bucketValues = [];
  for (let i = lo; i <= hi; i++) {
    const entry = distribution.top_values[i];
    if (entry) bucketValues.push(entry.value);
  }
  return bucketValues.length ? { columnName, bucketValues } : null;
}
