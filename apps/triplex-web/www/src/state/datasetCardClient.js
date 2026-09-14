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
 * stringified cells, any column type). Resolves to
 * `{ lDiversity, tCloseness, betaLikeness, deltaDisclosure, jointK, rowRisk }`:
 * the first four are each a `rows.length x cols.length` array of
 * `RiskAssessment | null` (one per quasi-identifier/candidate-sensitive-column
 * pair, the design notes adding the latter two alongside the original pair);
 * `jointK` is a `rows.length x rows.length` array — the dataset-level
 * verdict, every quasi-identifier column pair's *combined* k-anonymity
 * (`null` on the diagonal, a column paired with itself), not each one scored
 * alone. See `compose_quasi_identifiers` on the Rust side for why that's a
 * different computation from just averaging the per-column scores.
 * `rowRisk` is `[score, share][]`, one pair per row in original row order,
 * from whichever technique `rowRiskModel` names ("suda" or "ebm"
 *  — default "suda" if omitted) — `null` when there are no
 * quasi-identifier columns or more than `score_row_risk` accepts. */
export function scoreDiversityMatrix(rows, cols, bucketWidth, rowRiskModel) {
  return client.request("diversityMatrix", { rows, cols, bucketWidth, rowRiskModel });
}
