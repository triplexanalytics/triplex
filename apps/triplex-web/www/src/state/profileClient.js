// Thin request/response wrapper around the profiling worker — one
// outstanding request at a time is all this view needs (one uploaded file).
import { tableFromIPC } from "apache-arrow";

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
function sampleFromArrowTable(table) {
  const headers = table.schema.fields.map((f) => f.name);
  const columns = headers.map((name) => table.getChild(name));
  return {
    headers,
    rowCount: table.numRows,
    getCell(row, colIndex) {
      return cellToString(columns[colIndex]?.get(row));
    },
  };
}

function getWorker() {
  if (!worker) {
    worker = new Worker(new URL("../worker/profile.worker.js", import.meta.url), {
      type: "module",
    });
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
        console.log(
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
