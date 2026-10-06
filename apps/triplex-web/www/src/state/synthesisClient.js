// Thin request/response wrapper around synthesize.worker.js — see workerRequestClient.js for the
// shared shape.
import { createWorkerRequestClient } from "./workerRequestClient";

const client = createWorkerRequestClient(
  () => new Worker(new URL("../worker/synthesize.worker.js", import.meta.url), { type: "module" }),
);

/** Non-private synthesis. Resolves to `{ synthetic_csv, report_json }`. `rows` of 0 means as many
 * rows as the input. `excludedRowIndices`/`excludedColumns` are what the user removed in the table. */
export function runSynthesize({
  bytes,
  extension,
  seed,
  rows,
  minCategoryCount,
  excludedRowIndices,
  excludedColumns,
}) {
  return client.request("synthesize", {
    bytes,
    extension,
    seed,
    rows,
    minCategoryCount,
    excludedRowIndices,
    excludedColumns,
  });
}

/** Differentially private synthesis. `config` is the `synthesize:` block's JSON shape (see
 * synthesisConfig.js `buildDpConfig`). Resolves to `{ synthetic_csv, report_json }`. */
export function runSynthesizeDp({
  bytes,
  extension,
  config,
  seed,
  rows,
  withDiagnostics,
  excludedRowIndices,
}) {
  return client.request("synthesizeDp", {
    bytes,
    extension,
    config,
    seed,
    rows,
    withDiagnostics,
    excludedRowIndices,
  });
}

/** Resolves to a JSON string: per-column domain suggestions READ FROM THE DATA (so not public
 * bounds; see synthesisConfig.js). */
export function suggestDomains(bytes, extension) {
  return client.request("suggestDomains", { bytes, extension });
}
