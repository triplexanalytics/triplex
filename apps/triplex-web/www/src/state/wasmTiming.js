// Call-timing instrumentation for the wasm boundary — console.log only,
// nothing sent anywhere, so it needs no telemetry/data-egress ADR. Gated on
// Vite's own dev/prod flag (`import.meta.env.DEV`) rather than a bespoke
// toggle: on during `npm run dev`/`make dev`, stripped from a production
// build without needing to remember to remove call sites by hand later.
export function timingLog(message) {
  if (import.meta.env.DEV) console.log(message);
}

/** Times a single wasm call and logs it immediately. */
export function timedWasmCall(label, fn) {
  const start = performance.now();
  const result = fn();
  timingLog(`[wasm] ${label}: ${(performance.now() - start).toFixed(1)}ms`);
  return result;
}

/** Accumulates many small wasm calls (e.g. one matrix's worth of pairwise
 * scoring) into one summary line instead of flooding the console with one
 * line per call. Call `.call(fn)` for each wasm invocation, `.log(label)`
 * once at the end. */
export function wasmCallAccumulator() {
  let totalMs = 0;
  let count = 0;
  return {
    call(fn) {
      const start = performance.now();
      const result = fn();
      totalMs += performance.now() - start;
      count += 1;
      return result;
    },
    log(label) {
      timingLog(
        `[wasm] ${label}: ${count} call(s), ${totalMs.toFixed(1)}ms total, ` +
          `${count ? (totalMs / count).toFixed(2) : "0"}ms avg`,
      );
    },
  };
}
