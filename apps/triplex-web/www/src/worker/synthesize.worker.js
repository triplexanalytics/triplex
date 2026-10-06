// Runs the Tier 2 synthesis calls off the main thread: they fit a model to, and sample from,
// every row of the file (and the DP path is O(n^2) per numeric column pair). Same pattern as
// anonymize.worker.js: this worker gets its own wasm instance.
//
// Nothing here leaves the browser: the file's bytes, the synthetic CSV and the report only move
// between this worker and the page.
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
    if (type === "synthesize") {
      const output = wasm.synthesize(
        payload.bytes,
        payload.extension,
        payload.seed,
        payload.rows,
        payload.minCategoryCount,
        payload.excludedRowIndices ?? [],
        JSON.stringify(payload.excludedColumns ?? []),
      );
      result = { synthetic_csv: output.synthetic_csv, report_json: output.report_json };
    } else if (type === "synthesizeBayes") {
      const output = wasm.synthesize_bayes(
        payload.bytes,
        payload.extension,
        payload.seed,
        payload.rows,
        payload.minCategoryCount,
        payload.excludedRowIndices ?? [],
        JSON.stringify(payload.excludedColumns ?? []),
        payload.structure,
        payload.maxParents,
        payload.bins,
      );
      result = { synthetic_csv: output.synthetic_csv, report_json: output.report_json };
    } else if (type === "synthesizeDp") {
      const output = wasm.synthesize_dp(
        payload.bytes,
        payload.extension,
        JSON.stringify(payload.config),
        payload.seed,
        payload.rows,
        !!payload.withDiagnostics,
        payload.excludedRowIndices ?? [],
      );
      result = { synthetic_csv: output.synthetic_csv, report_json: output.report_json };
    } else if (type === "suggestDomains") {
      result = wasm.suggest_synthesis_domains(payload.bytes, payload.extension);
    } else {
      return;
    }
    timingLog(`[wasm worker] ${type}: ${(performance.now() - start).toFixed(1)}ms`);
    self.postMessage({ type: `${type}:ok`, requestId, result });
  } catch (err) {
    self.postMessage({ type: `${type}:error`, requestId, message: String(err) });
  }
};
