// Runs the whole-file table read and per-column profiling off the main
// thread. Gets its own independent wasm instance from the main thread's —
// cheap to instantiate, and nothing here needs shared memory with it.
import init, * as wasm from "../../pkg/triplex_web.js";
import wasmUrl from "../../pkg/triplex_web_bg.wasm?url";

const ready = init(wasmUrl);

self.onmessage = async (event) => {
  const { type, requestId, bytes, extension } = event.data;
  if (type !== "profile") return;

  try {
    await ready;
    const sampleJson = wasm.read_table(new Uint8Array(bytes), extension);
    const profileJson = wasm.profile_columns(new Uint8Array(bytes), extension);
    self.postMessage({ type: "profile:ok", requestId, sampleJson, profileJson });
  } catch (err) {
    self.postMessage({ type: "profile:error", requestId, message: String(err) });
  }
};
