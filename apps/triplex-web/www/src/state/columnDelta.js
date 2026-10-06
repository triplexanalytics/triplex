// How much one column changed between two datasets, for the Compare view's per-row strip. Same
// definitions the synthesis report uses: Kolmogorov-Smirnov distance for numbers, total variation
// distance for categories (0 = identical distributions, 1 = completely different).

function numericValues(sample, colIndex, excluded) {
  const all = sample.getNumericColumn(colIndex);
  const out = [];
  for (let i = 0; i < all.length; i++)
    if (!excluded?.has(i) && !Number.isNaN(all[i])) out.push(all[i]);
  return Float64Array.from(out).sort();
}

function categoryCounts(sample, colIndex, excluded) {
  const counts = new Map();
  let total = 0;
  for (let i = 0; i < sample.rowCount; i++) {
    if (excluded?.has(i)) continue;
    const cell = sample.getCell(i, colIndex);
    if (cell === "" || cell === null || cell === undefined) continue;
    counts.set(cell, (counts.get(cell) ?? 0) + 1);
    total++;
  }
  return { counts, total };
}

/** KS distance between two sorted arrays. */
export function ksDistance(a, b) {
  if (a.length === 0 || b.length === 0) return null;
  let i = 0;
  let j = 0;
  let worst = 0;
  while (i < a.length && j < b.length) {
    const x = Math.min(a[i], b[j]);
    while (i < a.length && a[i] <= x) i++;
    while (j < b.length && b[j] <= x) j++;
    worst = Math.max(worst, Math.abs(i / a.length - j / b.length));
  }
  return worst;
}

export function totalVariation(a, b) {
  if (a.total === 0 || b.total === 0) return null;
  let sum = 0;
  for (const key of new Set([...a.counts.keys(), ...b.counts.keys()])) {
    sum += Math.abs((a.counts.get(key) ?? 0) / a.total - (b.counts.get(key) ?? 0) / b.total);
  }
  return sum / 2;
}

/** `left`/`right` = `{ sample, kind: "numeric" | "categorical", excluded?: Set<number> }`. Returns
 * null when the two columns are not comparable (different kinds, or no values). */
export function distributionDifference(name, left, right) {
  if (left.kind !== right.kind) return null;
  const li = left.sample.headers.indexOf(name);
  const ri = right.sample.headers.indexOf(name);
  if (li < 0 || ri < 0) return null;
  if (left.kind === "numeric") {
    return ksDistance(
      numericValues(left.sample, li, left.excluded),
      numericValues(right.sample, ri, right.excluded),
    );
  }
  return totalVariation(
    categoryCounts(left.sample, li, left.excluded),
    categoryCounts(right.sample, ri, right.excluded),
  );
}
