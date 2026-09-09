// Shared shape for a "one worker, many named request types" client:
// datasetCardClient.js and anonymizeClient.js were byte-for-byte identical
// here (a `getWorker()` singleton keyed by a `pending` request-id Map, and
// a `request(type, payload)` helper resolving/rejecting on a `${type}:ok`/
// anything-else response). profileClient.js is deliberately not built on
// this: its response needs extra shaping (Arrow decode, not just resolving
// `result` as-is) and a timing log around that step, so forcing it through
// the same generic resolve path would need its own escape hatch anyway.
//
// `workerFactory` is called at most once (lazily, on first request) — a
// `() => new Worker(new URL(...), { type: "module" })` thunk, so the
// `import.meta.url` base stays correct at each call site.
export function createWorkerRequestClient(workerFactory) {
  let worker;
  let nextRequestId = 1;
  const pending = new Map();

  function getWorker() {
    if (!worker) {
      worker = workerFactory();
      worker.onmessage = (event) => {
        const { type, requestId, result, message } = event.data;
        const resolver = pending.get(requestId);
        if (!resolver) return;
        pending.delete(requestId);
        if (type.endsWith(":ok")) {
          resolver.resolve(result);
        } else {
          resolver.reject(new Error(message));
        }
      };
    }
    return worker;
  }

  function request(type, payload) {
    const requestId = nextRequestId++;
    return new Promise((resolve, reject) => {
      pending.set(requestId, { resolve, reject });
      getWorker().postMessage({ type, requestId, ...payload });
    });
  }

  return { request };
}
