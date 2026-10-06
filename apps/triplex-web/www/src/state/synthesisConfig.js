// Pure logic behind the Synthesize tab: turning the form's state into the config the wasm
// exports expect, and deciding whether a differentially private run may start. Kept out of
// components so the privacy-relevant rules are unit-tested (see synthesisConfig.test.js).
//
// The rule that matters: a DP run needs PUBLIC bounds, declared without
// looking at this data. The UI may prefill bounds from the data as a *suggestion*, but a column
// whose bounds still came from that suggestion blocks the run until the user confirms they are
// public knowledge.

/** Founder decision (2026-09-20): real-data diagnostics in DP mode are OFF until the user ticks a
 * box. Flip this one constant to change the default; it is not spread across components. */
export const DP_DIAGNOSTICS_DEFAULT = false;

export const DEFAULT_MIN_CATEGORY_COUNT = 5;
export const DEFAULT_BINS = 32;
export const DEFAULT_MARGINAL_SHARE = 0.5;
/** Mirrors `triplex_anonymizer::synthesize::dp::MAX_ROWS_WITH_CORRELATION` (the backend refuses
 * above it; this lets the UI say so before running). */
export const MAX_DP_ROWS_WITH_CORRELATION = 30000;
export const OTHER_CATEGORY = "(other)";
/** The noise sampler's epsilon resolution (`dp/noise.rs`): smaller budgets are refused. */
export const MIN_EPSILON = 0.000001;

/** Distinct, trimmed, non-empty values from comma- or newline-separated text. */
export function parseValueList(text) {
  const seen = new Set();
  const values = [];
  for (const raw of String(text ?? "").split(/[,\n]/)) {
    const v = raw.trim();
    if (v && !seen.has(v)) {
      seen.add(v);
      values.push(v);
    }
  }
  return values;
}

/** One editable row per column of the file. Nothing is prefilled from the data. */
export function blankColumns(headers, removedColumns = new Set()) {
  return headers.map((name) => ({
    name,
    include: false,
    kind: "numeric",
    min: "",
    max: "",
    integer: false,
    values: "",
    source: "typed",
    note: removedColumns.has(name) ? "removed from the output" : null,
  }));
}

/** Prefills rows from `wasm.suggest_synthesis_domains`' output. Suggested rows are marked
 * `source: "suggested"`: they came from the data, so they are not public bounds until confirmed.
 * Columns the user removed from the output are never pre-selected. */
export function applySuggestions(columns, suggestions, removedColumns = new Set()) {
  const byName = new Map(suggestions.map((s) => [s.name, s]));
  return columns.map((column) => {
    const s = byName.get(column.name);
    if (!s) return column;
    const removed = removedColumns.has(column.name);
    const include = !!s.suggested_include && !removed;
    const note = removed ? "removed from the output" : (s.note ?? null);
    if (s.type === "numeric") {
      return {
        ...column,
        kind: "numeric",
        min: String(s.min),
        max: String(s.max),
        integer: !!s.integer,
        values: "",
        include,
        source: "suggested",
        note,
      };
    }
    if (s.type === "categorical") {
      return {
        ...column,
        kind: "categorical",
        min: "",
        max: "",
        integer: false,
        values: s.values.join(", "),
        include,
        source: "suggested",
        note,
      };
    }
    return { ...column, include: false, note };
  });
}

const DOMAIN_FIELDS = ["kind", "min", "max", "integer", "values"];

/** Applies an edit to one column. Editing any part of its domain makes it user-declared
 * (`source: "typed"`); only toggling `include` leaves the source alone. */
export function updateColumn(columns, name, patch) {
  return columns.map((column) => {
    if (column.name !== name) return column;
    const touchesDomain = DOMAIN_FIELDS.some((f) => f in patch);
    return { ...column, ...patch, ...(touchesDomain ? { source: "typed" } : {}) };
  });
}

/** True when an included column still carries bounds that came from the data. */
export function needsPublicConfirmation(columns) {
  return columns.some((c) => c.include && c.source === "suggested");
}

/**
 * @returns {{ ok: boolean, errors: { field: string, message: string }[] }}
 * `rowCount` is the number of rows that will be used (after the user's row removals).
 */
export function validateDp({ epsilon, marginalShare, bins, columns, confirmedPublic, rowCount }) {
  const errors = [];
  const add = (field, message) => errors.push({ field, message });

  const eps = Number(epsilon);
  if (String(epsilon ?? "").trim() === "" || !Number.isFinite(eps) || eps <= 0) {
    add("epsilon", "Enter a privacy budget (epsilon) greater than 0. There is no default.");
  } else if (eps < MIN_EPSILON) {
    add("epsilon", `Epsilon must be at least ${MIN_EPSILON}.`);
  }
  const share = Number(marginalShare);
  if (!(share > 0 && share < 1)) {
    add("marginalShare", "The marginal share must be strictly between 0 and 1.");
  }
  const binCount = Number(bins);
  if (!Number.isInteger(binCount) || binCount < 2) {
    add("bins", "Use a whole number of bins, at least 2.");
  }

  const included = columns.filter((c) => c.include);
  if (included.length === 0) {
    add("columns", "Select at least one column and declare its public domain.");
  }
  for (const column of included) {
    if (column.kind === "numeric") {
      const min = Number(column.min);
      const max = Number(column.max);
      if (
        column.min.trim() === "" ||
        column.max.trim() === "" ||
        !Number.isFinite(min) ||
        !Number.isFinite(max)
      ) {
        add(`column:${column.name}`, `${column.name}: enter a minimum and a maximum.`);
      } else if (!(min < max)) {
        add(`column:${column.name}`, `${column.name}: the minimum must be below the maximum.`);
      }
    } else {
      const values = parseValueList(column.values);
      if (values.length === 0) {
        add(`column:${column.name}`, `${column.name}: list at least one category value.`);
      } else if (values.includes(OTHER_CATEGORY)) {
        add(`column:${column.name}`, `${column.name}: "${OTHER_CATEGORY}" is reserved.`);
      }
    }
  }
  const numericCount = included.filter((c) => c.kind === "numeric").length;
  if (numericCount >= 2 && rowCount > MAX_DP_ROWS_WITH_CORRELATION) {
    add(
      "rows",
      `With two or more numeric columns, DP mode supports at most ${MAX_DP_ROWS_WITH_CORRELATION.toLocaleString()} rows (you have ${rowCount.toLocaleString()}). Remove rows or use fewer numeric columns.`,
    );
  }
  if (needsPublicConfirmation(columns) && !confirmedPublic) {
    add(
      "confirmPublic",
      "Some bounds were suggested from your data. Confirm they are public knowledge, or type your own.",
    );
  }
  return { ok: errors.length === 0, errors };
}

/** The JSON the `synthesize_dp` wasm export expects (the `synthesize:` block's shape). */
export function buildDpConfig({ epsilon, marginalShare, bins, columns }) {
  return {
    epsilon: Number(epsilon),
    marginal_share: Number(marginalShare),
    bins: Number(bins),
    columns: columns
      .filter((c) => c.include)
      .map((c) =>
        c.kind === "numeric"
          ? {
              name: c.name,
              type: "numeric",
              min: Number(c.min),
              max: Number(c.max),
              integer: !!c.integer,
            }
          : { name: c.name, type: "categorical", values: parseValueList(c.values) },
      ),
  };
}

/** Checks both modes share: the sampling seed and the optional output row count. */
export function validateCommon({ seed, rows }) {
  const errors = [];
  const seedNumber = Number(seed);
  if (
    String(seed ?? "").trim() === "" ||
    !Number.isInteger(seedNumber) ||
    seedNumber < 0 ||
    seedNumber > 4294967295
  ) {
    errors.push({
      field: "seed",
      message: "The seed must be a whole number from 0 to 4294967295.",
    });
  }
  if (String(rows ?? "").trim() !== "") {
    const n = Number(rows);
    if (!Number.isInteger(n) || n < 1 || n > 4294967295) {
      errors.push({
        field: "rowsOut",
        message: "Leave rows blank, or enter a whole number of at least 1.",
      });
    }
  }
  return errors;
}

/** Rows the run will use: the file's rows minus those the user removed in the table. */
export function rowsInUse(totalRows, removedRowCount) {
  return Math.max(0, totalRows - removedRowCount);
}
