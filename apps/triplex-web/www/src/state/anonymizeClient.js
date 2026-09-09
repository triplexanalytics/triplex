// Thin request/response wrapper around anonymize.worker.js — see
// workerRequestClient.js for the shared shape.
import { createWorkerRequestClient } from "./workerRequestClient";

const client = createWorkerRequestClient(
  () => new Worker(new URL("../worker/anonymize.worker.js", import.meta.url), { type: "module" }),
);

/** Resolves to `{ masked_csv, mapping_csv, report_json }` — the same shape
 * `wasm.anonymize`'s `AnonymizeOutput` carries, just plain-object'd for the
 * worker boundary. */
export function runAnonymize(bytes, extension, recipeYaml, saltsJson) {
  return client.request("anonymize", { bytes, extension, recipeYaml, saltsJson });
}

/** Resolves to the `ValidationReport` JSON string. */
export function checkPatternConformance(bytes, extension, contractYaml) {
  return client.request("checkPatternConformance", { bytes, extension, contractYaml });
}
