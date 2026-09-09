// Runs the two wasm calls that scale with the *whole dataset* off the main
// thread — anonymize masks every declared column across every row, and
// check_pattern_conformance re-scans every row of the (already masked)
// output. Same pattern as profile.worker.js/datasetcard.worker.js: this
// worker gets its own wasm instance.
//
// contract_json_to_yaml/recipe_json_to_yaml/list_anonymize_methods stay on
// the main thread deliberately — they operate on the contract/recipe JSON,
// which has one entry per *column*, not per row, so it doesn't grow with
// file size the way these two calls do; moving them here would only add
// postMessage round-trip latency for no benefit.
import init, * as wasm from "../../pkg/triplex_web.js";
import wasmUrl from "../../pkg/triplex_web_bg.wasm?url";
import { timingLog } from "../state/wasmTiming";

const ready = init(wasmUrl);

self.onmessage = async (event) => {
  const { type, requestId, ...payload } = event.data;
  try {
    await ready;
    const start = performance.now();
    let result;
    if (type === "anonymize") {
      const output = wasm.anonymize(
        payload.bytes,
        payload.extension,
        payload.recipeYaml,
        payload.saltsJson,
      );
      result = {
        masked_csv: output.masked_csv,
        mapping_csv: output.mapping_csv,
        report_json: output.report_json,
      };
    } else if (type === "checkPatternConformance") {
      result = wasm.check_pattern_conformance(payload.bytes, payload.extension, payload.contractYaml);
    } else {
      return;
    }
    timingLog(`[wasm worker] ${type}: ${(performance.now() - start).toFixed(1)}ms`);
    self.postMessage({ type: `${type}:ok`, requestId, result });
  } catch (err) {
    self.postMessage({ type: `${type}:error`, requestId, message: String(err) });
  }
};
