// The one place this app creates its Web Workers. The Jupyter widget build
// (vite.widget.config.js) swaps this module for workers.widget.js, because a widget is a single
// JS file with no sibling files to load a worker from.
//
// Each `new Worker(new URL(...), import.meta.url)` is spelled out in full: Vite only bundles a
// worker when it sees that exact static pattern.
export const spawnProfileWorker = () =>
  new Worker(new URL("../worker/profile.worker.js", import.meta.url), { type: "module" });

export const spawnAnonymizeWorker = () =>
  new Worker(new URL("../worker/anonymize.worker.js", import.meta.url), { type: "module" });

export const spawnDatasetCardWorker = () =>
  new Worker(new URL("../worker/datasetcard.worker.js", import.meta.url), { type: "module" });

export const spawnSynthesizeWorker = () =>
  new Worker(new URL("../worker/synthesize.worker.js", import.meta.url), { type: "module" });
