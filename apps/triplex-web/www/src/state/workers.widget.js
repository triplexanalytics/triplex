// Widget-build stand-in for workers.js (swapped in by vite.widget.config.js). Workers are inlined
// into the bundle and started from blob URLs.
//
// The .wasm is inlined once, in the main bundle, as a base64 data URL. Each worker would
// otherwise inline its own copy (five copies of a 3.6 MB file), so the page turns its copy into
// one blob URL and hands that to every worker as the worker's `name`, which
// worker/wasmUrl.widget.js reads back.
import ProfileWorker from "../worker/profile.worker.js?worker&inline";
import AnonymizeWorker from "../worker/anonymize.worker.js?worker&inline";
import DatasetCardWorker from "../worker/datasetcard.worker.js?worker&inline";
import SynthesizeWorker from "../worker/synthesize.worker.js?worker&inline";
import wasmDataUrl from "../../pkg/triplex_web_bg.wasm?url";

let wasmBlobUrl;

function workerOptions() {
  if (!wasmBlobUrl) {
    const base64 = wasmDataUrl.slice(wasmDataUrl.indexOf(",") + 1);
    const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
    wasmBlobUrl = URL.createObjectURL(new Blob([bytes], { type: "application/wasm" }));
  }
  return { name: wasmBlobUrl };
}

export const spawnProfileWorker = () => new ProfileWorker(workerOptions());
export const spawnAnonymizeWorker = () => new AnonymizeWorker(workerOptions());
export const spawnDatasetCardWorker = () => new DatasetCardWorker(workerOptions());
export const spawnSynthesizeWorker = () => new SynthesizeWorker(workerOptions());
