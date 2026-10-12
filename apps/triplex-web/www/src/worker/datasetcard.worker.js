// Runs DatasetCard's risk-scoring wasm calls off the main thread — same
// reasoning and pattern as profile.worker.js: cheap to give this its own
// wasm instance, and the diversity matrix is O(rows x cols) wasm calls that
// have no business blocking the UI on a large file. Every message does its
// whole computation in one round trip (loops inside the worker), not one
// postMessage per pair — structured-clone overhead per call would dwarf the
// sub-millisecond wasm calls it's wrapping.
import init, * as wasm from "../../pkg/triplex_web.js";
import wasmUrl from "./wasmUrl";
import { timingLog } from "../state/wasmTiming";

const ready = init(wasmUrl);

function columnRisk(columns, bucketWidth) {
  const result = {};
  for (const { name, kind, values } of columns) {
    result[name] =
      kind === "string"
        ? JSON.parse(wasm.score_column_risk_string(JSON.stringify(values), 0))
        : JSON.parse(wasm.score_column_risk(values, bucketWidth));
  }
  return result;
}

function diversityMatrix(rows, cols, bucketWidth, rowRiskModel) {
  // Each col's sensitive values get stringified once here, not once per
  // (row, col) pair inside the loop below — cols.length JSON.stringify
  // calls total instead of rows.length x cols.length redundant ones of
  // the same unchanged array.
  const colsJson = cols.map((col) => JSON.stringify(col.values));
  const lDiversity = [];
  const tCloseness = [];
  const betaLikeness = [];
  const deltaDisclosure = [];
  for (const row of rows) {
    const lRow = [];
    const tRow = [];
    const betaRow = [];
    const deltaRow = [];
    cols.forEach((col, j) => {
      if (col.name === row.name) {
        lRow.push(null);
        tRow.push(null);
        betaRow.push(null);
        deltaRow.push(null);
        return;
      }
      // row.values is a typed Float64Array for a numeric row (no stringify
      // needed, wasm-bindgen takes it directly) or a plain string array for
      // a string row, stringified the same way cols already are. Both wasm
      // calls now return [l, t, beta, delta].
      const [l, t, beta, delta] =
        row.kind === "string"
          ? JSON.parse(
              wasm.score_column_pair_risk_string(JSON.stringify(row.values), colsJson[j], 0),
            )
          : JSON.parse(wasm.score_column_pair_risk(row.values, colsJson[j], bucketWidth));
      // Each model's own worst-scoring quasi-identifier bucket -- same
      // "which group" pairing jointKMatrix does for combined k-anonymity,
      // letting a click on this cell filter the table to exactly that
      // group instead of just showing its severity.
      const [lWorst, tWorst, betaWorst, deltaWorst] =
        row.kind === "string"
          ? JSON.parse(
              wasm.worst_group_column_pair_risk_string(JSON.stringify(row.values), colsJson[j], 0),
            )
          : JSON.parse(wasm.worst_group_column_pair_risk(row.values, colsJson[j], bucketWidth));
      l.worstGroup = lWorst ?? null;
      t.worstGroup = tWorst ?? null;
      beta.worstGroup = betaWorst ?? null;
      delta.worstGroup = deltaWorst ?? null;
      lRow.push(l);
      tRow.push(t);
      betaRow.push(beta);
      deltaRow.push(delta);
    });
    lDiversity.push(lRow);
    tCloseness.push(tRow);
    betaLikeness.push(betaRow);
    deltaDisclosure.push(deltaRow);
  }
  const bucketed = bucketQuasiIdentifiers(rows, bucketWidth);
  return {
    lDiversity,
    tCloseness,
    betaLikeness,
    deltaDisclosure,
    jointK: jointKMatrix(bucketed),
    rowRisk: rowRisk(bucketed, rowRiskModel),
  };
}

// Each quasi-identifier column generalized once (numeric -> "lo-hi"
// buckets via `bucket_numeric_column`, string -> exact values) and reused
// by both `jointKMatrix` and `rowRisk` below, since neither's wasm call
// wants raw values -- bucketing width doesn't change per pair or per row.
function bucketQuasiIdentifiers(rows, bucketWidth) {
  return rows.map(({ kind, values }) =>
    kind === "string" ? values : Array.from(wasm.bucket_numeric_column(values, bucketWidth)),
  );
}

// The dataset-level verdict (the design notes "dataset-level
// aggregation" gap): every quasi-identifier column pair's *combined*
// k-anonymity, not just each one scored alone -- two columns that each
// look safe independently (a ZIP code, a birth year) can single out
// almost everyone once combined.
function jointKMatrix(bucketed) {
  const n = bucketed.length;
  const matrix = Array.from({ length: n }, () => new Array(n).fill(null));
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      // Symmetric: combining column i with j scores the same as j with i,
      // so compute each pair once and mirror it instead of two wasm calls.
      const pairJson = JSON.stringify([bucketed[i], bucketed[j]]);
      const cell = JSON.parse(wasm.score_joint_column_risk(pairJson));
      // The smallest equivalence class's own composite key (column i's
      // bucket joined to column j's by U+001F, `compose_quasi_identifiers`'s
      // convention) -- lets a click on this cell filter the table down to
      // exactly the rows in that class instead of just showing its size.
      cell.worstGroup = wasm.worst_group_joint_column_risk(pairJson) ?? null;
      matrix[i][j] = cell;
      matrix[j][i] = cell;
    }
  }
  return matrix;
}

// Per-row disclosure risk: which specific rows are
// most identifiable across every quasi-identifier column at once, a
// different question from any matrix above (those all answer "how safe is
// this column/pair," not "which rows should I actually worry about").
// `rowRiskModel` picks the technique ("suda" or "ebm", see
// `score_row_risk`'s doc comment on the Rust side) -- both return the same
// `[score, share]`-per-row shape, just on different numeric scales. `null`
// when there are no quasi-identifier columns to score, or when there are
// more than SUDA accepts (`MAX_QUASI_IDENTIFIER_COLUMNS`, see its doc
// comment -- EBM has no such cap, but the two techniques are switched
// together, not independently limited) -- degrades to "not shown" rather
// than failing the whole diversity matrix over a row-risk-only limit.
function rowRisk(bucketed, rowRiskModel) {
  if (bucketed.length === 0 || bucketed.length > 10) return null;
  return JSON.parse(wasm.score_row_risk(JSON.stringify(bucketed), rowRiskModel ?? "suda"));
}

self.onmessage = async (event) => {
  const { type, requestId, ...payload } = event.data;
  try {
    await ready;
    const start = performance.now();
    let result;
    if (type === "columnRisk") {
      result = columnRisk(payload.columns, payload.bucketWidth);
    } else if (type === "diversityMatrix") {
      result = diversityMatrix(
        payload.rows,
        payload.cols,
        payload.bucketWidth,
        payload.rowRiskModel,
      );
    } else {
      return;
    }
    timingLog(`[wasm worker] ${type}: ${(performance.now() - start).toFixed(1)}ms`);
    self.postMessage({ type: `${type}:ok`, requestId, result });
  } catch (err) {
    self.postMessage({ type: `${type}:error`, requestId, message: String(err) });
  }
};
