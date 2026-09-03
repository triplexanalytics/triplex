// Thin request/response wrapper around the profiling worker — one
// outstanding request at a time is all this view needs (one uploaded file).
let worker;
let nextRequestId = 1;
const pending = new Map();

function getWorker() {
  if (!worker) {
    worker = new Worker(new URL("../worker/profile.worker.js", import.meta.url), {
      type: "module",
    });
    worker.onmessage = (event) => {
      const { type, requestId, ...rest } = event.data;
      const resolver = pending.get(requestId);
      if (!resolver) return;
      pending.delete(requestId);
      if (type === "profile:ok") {
        resolver.resolve({
          sample: JSON.parse(rest.sampleJson),
          profile: JSON.parse(rest.profileJson),
        });
      } else {
        resolver.reject(new Error(rest.message));
      }
    };
  }
  return worker;
}

/** Reads and profiles an uploaded file's bytes off the main thread. */
export function profileFile(bytes, extension) {
  const requestId = nextRequestId++;
  return new Promise((resolve, reject) => {
    pending.set(requestId, { resolve, reject });
    // Copied (structured clone), not transferred — the main thread still
    // needs `bytes` afterward (e.g. for the Anonymize call), and a
    // browser-picked file is small enough that transfer would be a
    // premature optimization.
    getWorker().postMessage({ type: "profile", requestId, bytes, extension });
  });
}
