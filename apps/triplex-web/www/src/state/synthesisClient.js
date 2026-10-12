// Thin request/response wrapper around synthesize.worker.js — see workerRequestClient.js for the
// shared shape.
import { createWorkerRequestClient } from "./workerRequestClient";
import { spawnSynthesizeWorker } from "./workers";

const client = createWorkerRequestClient(spawnSynthesizeWorker);

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

/** Non-private Bayesian network. `structure` is "chow_liu" or "k_parent";
 * `maxParents` (1 to 3) only matters for "k_parent". Resolves to `{ synthetic_csv, report_json }`. */
export function runSynthesizeBayes({
  bytes,
  extension,
  seed,
  rows,
  minCategoryCount,
  excludedRowIndices,
  excludedColumns,
  structure,
  maxParents,
  bins,
}) {
  return client.request("synthesizeBayes", {
    bytes,
    extension,
    seed,
    rows,
    minCategoryCount,
    excludedRowIndices,
    excludedColumns,
    structure,
    maxParents,
    bins,
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

/** Tier 3: proposes a sketch for the file. Resolves to the sketch as a JSON string. */
export function proposeSketch(payload) {
  return client.request("proposeSketch", payload);
}

/** Tier 3: samples from `sketch` (as proposed or after steering). Resolves to
 * `{ synthetic_csv, report_json }`. */
export function runSynthesizeSketch(payload) {
  return client.request("synthesizeSketch", payload);
}

/** Tier 3 steering: classes `a` and `b` become one. Resolves to the new sketch as a JSON string. */
export function mergeSketchClasses(sketch, a, b) {
  return client.request("mergeSketchClasses", { sketch, a, b });
}
