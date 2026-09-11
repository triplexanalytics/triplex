// Runs DatasetCard's risk-scoring wasm calls off the main thread — same
// reasoning and pattern as profile.worker.js: cheap to give this its own
// wasm instance, and the diversity matrix is O(rows x cols) wasm calls that
// have no business blocking the UI on a large file. Every message does its
// whole computation in one round trip (loops inside the worker), not one
// postMessage per pair — structured-clone overhead per call would dwarf the
// sub-millisecond wasm calls it's wrapping.
import init, * as wasm from "../../pkg/triplex_web.js";
import wasmUrl from "../../pkg/triplex_web_bg.wasm?url";
import { timingLog } from "../state/wasmTiming";

const ready = init(wasmUrl);

function columnRisk(columns, bucketWidth) {
  const result = {};
  for (const { name, kind, values } of columns) {
    result[name] =
      kind === "string"
        ? JSON.parse(wasm.score_column_risk_string(JSON.stringify(values)))
        : JSON.parse(wasm.score_column_risk(values, bucketWidth));
  }
  return result;
}

function diversityMatrix(rows, cols, bucketWidth) {
  // Each col's sensitive values get stringified once here, not once per
  // (row, col) pair inside the loop below — cols.length JSON.stringify
  // calls total instead of rows.length x cols.length redundant ones of
  // the same unchanged array.
  const colsJson = cols.map((col) => JSON.stringify(col.values));
  const lDiversity = [];
  const tCloseness = [];
  for (const row of rows) {
    const lRow = [];
    const tRow = [];
    cols.forEach((col, j) => {
      if (col.name === row.name) {
        lRow.push(null);
        tRow.push(null);
        return;
      }
      // row.values is a typed Float64Array for a numeric row (no stringify
      // needed, wasm-bindgen takes it directly) or a plain string array for
      // a string row, stringified the same way cols already are.
      const [l, t] =
        row.kind === "string"
          ? JSON.parse(wasm.score_column_pair_risk_string(JSON.stringify(row.values), colsJson[j]))
          : JSON.parse(wasm.score_column_pair_risk(row.values, colsJson[j], bucketWidth));
      lRow.push(l);
      tRow.push(t);
    });
    lDiversity.push(lRow);
    tCloseness.push(tRow);
  }
  return { lDiversity, tCloseness };
}

// Combined quasi-identifier check: `columns` (2+ picked columns, same
// `{ name, kind, values }` shape as `columnRisk`) get generalized the same
// way a single column would (numeric -> "lo-hi" buckets via
// `bucket_numeric_column`, string -> exact values), then scored together
// as one quasi-identifier by the wasm side's `compose_quasi_identifiers`
// — not scored independently and combined here, since the whole point is
// that the *joint* equivalence classes can be smaller than any one
// column's own classes. `sensitiveValues` (stringified cells for the
// chosen sensitive column) is optional: k-anonymity alone needs no
// sensitive column, l-diversity/t-closeness do.
function jointRisk(columns, bucketWidth, sensitiveValues) {
  const bucketedJson = JSON.stringify(
    columns.map(({ kind, values }) =>
      kind === "string" ? values : Array.from(wasm.bucket_numeric_column(values, bucketWidth)),
    ),
  );
  const kAnonymity = JSON.parse(wasm.score_joint_column_risk(bucketedJson));
  if (!sensitiveValues) return { kAnonymity, lDiversity: null, tCloseness: null };
  const [lDiversity, tCloseness] = JSON.parse(
    wasm.score_joint_column_pair_risk(bucketedJson, JSON.stringify(sensitiveValues)),
  );
  return { kAnonymity, lDiversity, tCloseness };
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
      result = diversityMatrix(payload.rows, payload.cols, payload.bucketWidth);
    } else if (type === "jointRisk") {
      result = jointRisk(payload.columns, payload.bucketWidth, payload.sensitiveValues);
    } else {
      return;
    }
    timingLog(`[wasm worker] ${type}: ${(performance.now() - start).toFixed(1)}ms`);
    self.postMessage({ type: `${type}:ok`, requestId, result });
  } catch (err) {
    self.postMessage({ type: `${type}:error`, requestId, message: String(err) });
  }
};
