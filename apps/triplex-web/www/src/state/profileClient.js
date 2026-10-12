// Thin request/response wrapper around the profiling worker — one
// outstanding request at a time is all this view needs (one uploaded file).
import { tableFromIPC } from "apache-arrow";
import { timingLog } from "./wasmTiming";
import { spawnProfileWorker } from "./workers";

let worker;
let nextRequestId = 1;
const pending = new Map();

function cellToString(value) {
  return value === null || value === undefined ? "" : String(value);
}

/** Wraps a decoded Arrow `Table` in the `{ headers, rowCount, getCell }`
 * shape the Table tab reads. Deliberately not re-expanded into a
 * `string[][]` — the whole point of sending row data as Arrow IPC instead
 * of a JSON string is to keep it typed and columnar all the way to render,
 * not copy it back out into one big array of stringified cells right after
 * decoding it. A numeric column's cells format as `String(value)` (e.g.
 * `"9.5"`, not the source text `"9.50"`) — Arrow stores it as a real
 * `Float64`/`Int64`, not the original string, same tradeoff the Parquet
 * conversion path already makes and documents. */
export function sampleFromArrowTable(table) {
  const headers = table.schema.fields.map((f) => f.name);
  const columns = headers.map((name) => table.getChild(name));
  // useColumnRisk, useDiversityMatrix, useRowFilter, and
  // useColumnSubsetDistributions each read the same quasi-identifier
  // columns independently, around the same triggers (file load, a row
  // removal) -- without this, every one of them rebuilt its own full
  // O(rows) Float64Array from the Arrow column from scratch. Cached per
  // colIndex, for this sample's lifetime only (a new upload makes a new
  // `sample`, so there's nothing to invalidate by hand).
  const numericColumnCache = new Map();
  return {
    headers,
    rowCount: table.numRows,
    getCell(row, colIndex) {
      return cellToString(columns[colIndex]?.get(row));
    },
    // Raw numeric values as a typed Float64Array (NaN for null/non-numeric)
    // instead of stringified cells — the risk-scoring wasm calls
    // (score_column_risk/score_column_pair_risk) take a typed array
    // directly, not a JSON string, so callers that already know a column
    // is numeric (every current one pre-filters to integer-inferred
    // columns) should read it this way, not through getCell + reparse.
    // Returns the cached array itself, not a copy -- callers must treat it
    // as read-only and must never hand its `.buffer` to a worker's
    // postMessage transfer list (that would detach it out from under every
    // other reader); build a fresh copy first if a call site needs either.
    getNumericColumn(colIndex) {
      const cached = numericColumnCache.get(colIndex);
      if (cached) return cached;
      const col = columns[colIndex];
      const out = new Float64Array(table.numRows);
      for (let row = 0; row < table.numRows; row++) {
        const v = col?.get(row);
        out[row] = v === null || v === undefined ? NaN : Number(v);
      }
      numericColumnCache.set(colIndex, out);
      return out;
    },
  };
}

function getWorker() {
  if (!worker) {
    worker = spawnProfileWorker();
    worker.onmessage = (event) => {
      const { type, requestId, ...rest } = event.data;
      const resolver = pending.get(requestId);
      if (!resolver) return;
      pending.delete(requestId);
      if (type === "profile:ok") {
        const t0 = performance.now();
        const table = tableFromIPC(rest.tableArrow);
        const sample = sampleFromArrowTable(table);
        const profile = JSON.parse(rest.profileJson);
        timingLog(
          `[main] postMessage round trip done, tableFromIPC + JSON.parse(profile): ` +
            `${(performance.now() - t0).toFixed(1)}ms`,
        );
        resolver.resolve({ sample, profile });
      } else {
        resolver.reject(new Error(rest.message));
      }
    };
  }
  return worker;
}

/** Reads and profiles an uploaded file's bytes off the main thread. */
export function profileFile(bytes, extension) {
  const requestId = nextRequestId++;
  return new Promise((resolve, reject) => {
    pending.set(requestId, { resolve, reject });
    // Copied (structured clone), not transferred — the main thread still
    // needs `bytes` afterward (e.g. for the Anonymize call), and a
    // browser-picked file is small enough that transfer would be a
    // premature optimization.
    getWorker().postMessage({ type: "profile", requestId, bytes, extension });
  });
}
