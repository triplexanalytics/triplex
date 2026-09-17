// Runs the whole-file table read and per-column profiling off the main
// thread. Gets its own independent wasm instance from the main thread's —
// cheap to instantiate, and nothing here needs shared memory with it.
import init, * as wasm from "../../pkg/triplex_web.js";
import wasmUrl from "../../pkg/triplex_web_bg.wasm?url";
import { timingLog } from "../state/wasmTiming";

const ready = init(wasmUrl);

self.onmessage = async (event) => {
  const { type, requestId, bytes, extension } = event.data;
  if (type !== "profile") return;

  try {
    await ready;
    const t0 = performance.now();
    const result = wasm.read_and_profile_table(new Uint8Array(bytes), extension);
    timingLog(
      `[wasm worker] read_and_profile_table: ${(performance.now() - t0).toFixed(1)}ms ` +
        `(${result.table_arrow.length} arrow bytes, ${result.profile_json.length} profile chars) ` +
        `for ${bytes.length} input bytes`,
    );
    // table_arrow is a Vec<u8> wasm-bindgen copied out of wasm memory into a
    // fresh Uint8Array/ArrayBuffer, not a view into live wasm memory — safe
    // to transfer (zero-copy handoff), since nothing here needs it again.
    self.postMessage(
      {
        type: "profile:ok",
        requestId,
        profileJson: result.profile_json,
        tableArrow: result.table_arrow,
      },
      // @ts-expect-error -- the (message, transferList) worker overload of
      // postMessage isn't visible here: DOM and WebWorker lib types can't
      // both be loaded in one tsconfig (they define conflicting globals),
      // and this app's other files need DOM. TS instead resolves this to
      // Window.postMessage's (message, targetOrigin, transfer?) overload,
      // which doesn't accept an array as the 2nd argument.
      [result.table_arrow.buffer],
    );
  } catch (err) {
    self.postMessage({ type: "profile:error", requestId, message: String(err) });
  }
};
