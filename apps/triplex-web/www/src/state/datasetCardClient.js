// Thin request/response wrapper around the DatasetCard scoring worker —
// see workerRequestClient.js for the shared shape. Multiple outstanding
// requests are possible here (a column-risk request and a
// diversity-matrix request can both be in flight for the same upload),
// which the shared client already handles by keying on request id rather
// than assuming one at a time.
import { createWorkerRequestClient } from "./workerRequestClient";

const client = createWorkerRequestClient(
  () => new Worker(new URL("../worker/datasetcard.worker.js", import.meta.url), { type: "module" }),
);

/** `columns`: `[{ name, kind: "numeric" | "string", values }]`, `values` a typed
 * `Float64Array` for `"numeric"` or a plain string array for `"string"`.
 * Resolves to `{ [name]: RiskAssessment }`. */
export function scoreColumnRisk(columns, bucketWidth) {
  return client.request("columnRisk", { columns, bucketWidth });
}

/** `rows`: `[{ name, kind: "numeric" | "string", values }]`, same `values`
 * shape as `scoreColumnRisk`'s `columns`. `cols`: `[{ name, values }]` (always
 * stringified cells, any column type). Resolves to `{ lDiversity, tCloseness }`,
 * each a `rows.length x cols.length` array of `RiskAssessment | null`. */
export function scoreDiversityMatrix(rows, cols, bucketWidth) {
  return client.request("diversityMatrix", { rows, cols, bucketWidth });
}
