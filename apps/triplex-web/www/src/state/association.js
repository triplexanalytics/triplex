// Column-to-column association for the Columns tab and the Compare view. One number per column
// pair on a 0..1 strength scale, with the measure chosen by the two column kinds:
//   number  x number      Pearson r (signed; strength = |r|)
//   category x category   Cramer's V (Bergsma bias correction, 0..1)
//   category x number     correlation ratio eta (0..1)
// The scales differ, which is why every cell carries its measure; a UI must say so.
//
// ponytail: one synchronous main-thread pass, O(rows) per pair. Move to a worker if a very wide or
// many-row table lags (same cost model as useColumnSubsetDistributions).

/** A column takes part when the profile calls it numeric or categorical and it is not an
 * identifier (an identifier's "association" with anything is noise). */
export function associationKind(col) {
  if (col.semantic_type === "likely_identifier") return null;
  const kind = col.distribution?.kind;
  return kind === "numeric" || kind === "categorical" ? kind : null;
}

function includedRows(rowCount, excluded) {
  const rows = [];
  for (let i = 0; i < rowCount; i++) if (!excluded?.has(i)) rows.push(i);
  return rows;
}

function readColumn(sample, colIndex, kind, rows) {
  if (kind === "numeric") {
    const all = sample.getNumericColumn(colIndex);
    return { kind, values: Float64Array.from(rows, (r) => all[r]) };
  }
  const ids = new Map();
  const codes = new Int32Array(rows.length);
  rows.forEach((r, n) => {
    const cell = sample.getCell(r, colIndex);
    if (cell === "" || cell === null || cell === undefined) return void (codes[n] = -1);
    if (!ids.has(cell)) ids.set(cell, ids.size);
    codes[n] = ids.get(cell);
  });
  return { kind, codes, levels: ids.size };
}

// A category column with more distinct values than this (an email, a free-text name) has no
// meaningful association with anything; Cramer's V and eta would both drift toward 1.
export const MAX_CATEGORY_LEVELS = 50;

export function pearson(a, b) {
  let n = 0;
  let sa = 0;
  let sb = 0;
  for (let i = 0; i < a.length; i++) {
    if (Number.isNaN(a[i]) || Number.isNaN(b[i])) continue;
    n++;
    sa += a[i];
    sb += b[i];
  }
  if (n < 2) return null;
  const ma = sa / n;
  const mb = sb / n;
  let sab = 0;
  let saa = 0;
  let sbb = 0;
  for (let i = 0; i < a.length; i++) {
    if (Number.isNaN(a[i]) || Number.isNaN(b[i])) continue;
    sab += (a[i] - ma) * (b[i] - mb);
    saa += (a[i] - ma) ** 2;
    sbb += (b[i] - mb) ** 2;
  }
  return saa === 0 || sbb === 0 ? null : sab / Math.sqrt(saa * sbb);
}

export function cramersV(x, y) {
  const table = new Map();
  const rowTotals = new Map();
  const colTotals = new Map();
  let n = 0;
  for (let i = 0; i < x.codes.length; i++) {
    if (x.codes[i] < 0 || y.codes[i] < 0) continue;
    n++;
    const key = x.codes[i] * y.levels + y.codes[i];
    table.set(key, (table.get(key) ?? 0) + 1);
    rowTotals.set(x.codes[i], (rowTotals.get(x.codes[i]) ?? 0) + 1);
    colTotals.set(y.codes[i], (colTotals.get(y.codes[i]) ?? 0) + 1);
  }
  const r = rowTotals.size;
  const k = colTotals.size;
  if (n < 2 || r < 2 || k < 2) return null;
  let chi2 = 0;
  for (const [rc, rt] of rowTotals) {
    for (const [cc, ct] of colTotals) {
      const expected = (rt * ct) / n;
      const observed = table.get(rc * y.levels + cc) ?? 0;
      chi2 += (observed - expected) ** 2 / expected;
    }
  }
  const phi2 = Math.max(0, chi2 / n - ((k - 1) * (r - 1)) / (n - 1));
  const rCorr = r - (r - 1) ** 2 / (n - 1);
  const kCorr = k - (k - 1) ** 2 / (n - 1);
  const denom = Math.min(kCorr - 1, rCorr - 1);
  return denom <= 0 ? 0 : Math.sqrt(phi2 / denom);
}

export function correlationRatio(cat, num) {
  const sums = new Map();
  let n = 0;
  let total = 0;
  for (let i = 0; i < cat.codes.length; i++) {
    const v = num.values[i];
    if (cat.codes[i] < 0 || Number.isNaN(v)) continue;
    n++;
    total += v;
    const s = sums.get(cat.codes[i]) ?? { count: 0, sum: 0 };
    s.count++;
    s.sum += v;
    sums.set(cat.codes[i], s);
  }
  if (n < 2 || sums.size < 2) return null;
  const grand = total / n;
  let between = 0;
  for (const { count, sum } of sums.values()) between += count * (sum / count - grand) ** 2;
  let within = 0;
  for (let i = 0; i < cat.codes.length; i++) {
    const v = num.values[i];
    if (cat.codes[i] >= 0 && !Number.isNaN(v)) within += (v - grand) ** 2;
  }
  return within === 0 ? null : Math.sqrt(Math.min(1, between / within));
}

function pairCell(a, b) {
  if (a.kind === "numeric" && b.kind === "numeric") {
    const r = pearson(a.values, b.values);
    return r === null ? null : { value: Math.abs(r), signed: r, measure: "pearson" };
  }
  if (a.kind === "categorical" && b.kind === "categorical") {
    const v = cramersV(a, b);
    return v === null ? null : { value: v, signed: null, measure: "cramers_v" };
  }
  const [cat, num] = a.kind === "categorical" ? [a, b] : [b, a];
  const eta = correlationRatio(cat, num);
  return eta === null ? null : { value: eta, signed: null, measure: "eta" };
}

/** `{ names, kinds, cells }`, `cells[i][j]` = `{ value (0..1), signed (Pearson only, else null),
 * measure }` or null (diagonal, or no usable rows). Rows in `excludedRows` are ignored. */
export function computeAssociation(sample, profile, excludedRows) {
  const eligible = profile.columns
    .map((col) => ({
      name: col.name,
      kind: associationKind(col),
      index: sample.headers.indexOf(col.name),
    }))
    .filter((c) => c.kind && c.index >= 0);
  const rows = includedRows(sample.rowCount, excludedRows);
  const read = eligible.map((c) => readColumn(sample, c.index, c.kind, rows));
  const keep = eligible.map(
    (_, i) => read[i].kind !== "categorical" || read[i].levels <= MAX_CATEGORY_LEVELS,
  );
  const columns = eligible.filter((_, i) => keep[i]);
  const data = read.filter((_, i) => keep[i]);
  const cells = data.map((_, i) =>
    data.map((_, j) => (i === j ? null : pairCell(data[i], data[j]))),
  );
  // The pair function is symmetric; computing both triangles keeps the code trivial. ponytail:
  // compute one triangle and mirror it if this ever shows up in a profile.
  return { names: columns.map((c) => c.name), kinds: columns.map((c) => c.kind), cells };
}

export const MEASURE_LABEL = {
  pearson: "Pearson r",
  cramers_v: "Cramér's V",
  eta: "correlation ratio",
};
