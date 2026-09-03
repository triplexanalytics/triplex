import init, * as wasm from "../pkg/triplex_web.js";
import wasmUrl from "../pkg/triplex_web_bg.wasm?url";

// Loaded explicitly via `?url` rather than the glue's own default
// `import.meta.url` resolution — one less thing that can differ between
// `vite dev`, `vite build`, and the Web Worker's separate module graph.
let ready;

export function initWasm() {
  if (!ready) {
    ready = init(wasmUrl).then(() => wasm);
  }
  return ready;
}
