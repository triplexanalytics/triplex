// The quasi-identifiers an original and its anonymized output are compared on. Both tables must
// be measured on the same columns, or a lower risk on one side could just mean it was measured on
// fewer columns.

/** A column the privacy matrices can treat as a quasi-identifier: a whole number or a category. */
export function isQuasiIdentifierKind(col) {
  return col.inferred_type === "integer" || col.distribution?.kind === "categorical";
}

/** A column that tells people apart on its own (an ID, an email address, a pseudonym): combining
 * it with anything singles out nearly everyone, which drowns out what the other columns do. */
function isIdentifierLike(col, rowCount) {
  if (col.semantic_type === "likely_identifier" || col.semantic_type === "email") return true;
  const cardinality = col.distribution?.cardinality;
  return typeof cardinality === "number" && cardinality > rowCount / 2;
}

/** The columns both profiles can treat as quasi-identifiers, in the left profile's order. `chosen`
 * is the subset picked by default: the identifier-like columns are left out of it. */
export function sharedQuasiIdentifiers(leftProfile, rightProfile, rowCount) {
  const rightByName = new Map(rightProfile.columns.map((c) => [c.name, c]));
  const candidates = [];
  const chosen = [];
  for (const left of leftProfile.columns) {
    const right = rightByName.get(left.name);
    if (!right || !isQuasiIdentifierKind(left) || !isQuasiIdentifierKind(right)) continue;
    candidates.push(left.name);
    if (!isIdentifierLike(left, rowCount) && !isIdentifierLike(right, rowCount)) {
      chosen.push(left.name);
    }
  }
  return { candidates, chosen };
}

/** How many rows share their exact combination of values in `names` with no other row, out of
 * the rows not in `excluded`. Values are compared exactly as the table holds them, so a whole
 * number is its own group (not a range) and a generalized value such as "1950-1959" groups every
 * row that carries it. `sample` is the `{ headers, rowCount, getCell }` table shape. */
export function rowsAlone(sample, names, excluded) {
  const indices = names.map((name) => sample.headers.indexOf(name));
  const counts = new Map();
  const keys = [];
  for (let row = 0; row < sample.rowCount; row++) {
    if (excluded?.has(row)) continue;
    // U+001F (unit separator) cannot occur in a parsed cell, so the joined key is unambiguous.
    const key = indices.map((i) => sample.getCell(row, i)).join("\u001f");
    keys.push(key);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  let alone = 0;
  for (const key of keys) if (counts.get(key) === 1) alone++;
  return { alone, total: keys.length };
}
