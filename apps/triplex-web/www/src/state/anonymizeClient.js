// Thin request/response wrapper around anonymize.worker.js — see
// workerRequestClient.js for the shared shape.
import { createWorkerRequestClient } from "./workerRequestClient";

const client = createWorkerRequestClient(
  () => new Worker(new URL("../worker/anonymize.worker.js", import.meta.url), { type: "module" }),
);

/** Resolves to `{ masked_csv, mapping_csv, report_json }` — the same shape
 * `wasm.anonymize`'s `AnonymizeOutput` carries, just plain-object'd for the
 * worker boundary. `excludedRowIndices` (0-based, from useProfiledFile.js's
 * `removedRowIndices` — "remove these rows from the output") defaults to
 * none excluded. */
export function runAnonymize(bytes, extension, recipeYaml, saltsJson, excludedRowIndices) {
  return client.request("anonymize", {
    bytes,
    extension,
    recipeYaml,
    saltsJson,
    excludedRowIndices,
  });
}

/** Resolves to the `ValidationReport` JSON string. */
export function checkPatternConformance(bytes, extension, contractYaml) {
  return client.request("checkPatternConformance", { bytes, extension, contractYaml });
}

/** Resolves to the re-encoded file as a `Uint8Array` — the "Download data"
 * format picker's conversion, off the main thread since it re-parses and
 * re-serializes every row like `runAnonymize` does. */
export function convertBytes(bytes, fromExtension, toExtension) {
  return client.request("convertBytes", { bytes, fromExtension, toExtension });
}
