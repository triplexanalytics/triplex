// The datasets the demo can show, and nothing else: the demo has no way to load a file of the
// visitor's own. Each file is a separate chunk, fetched from the demo's own origin when picked.

/**
 * @typedef {object} DemoDataset
 * @property {string} id         used in the address, `#dataset=<id>`
 * @property {string} label      shown in the picker
 * @property {string} fileName   the name the app shows and downloads under
 * @property {() => Promise<string>} load  the CSV text
 */

/** @type {DemoDataset[]} */
export const DATASETS = [
  {
    id: "municipal",
    label: "Municipal benefits register",
    fileName: "municipal.csv",
    load: () =>
      import("../../../../../tests/fixtures/demo/municipal.csv?raw").then((m) => m.default),
  },
  {
    id: "statistical",
    label: "Statistical microdata",
    fileName: "statistical.csv",
    load: () =>
      import("../../../../../tests/fixtures/demo/statistical.csv?raw").then((m) => m.default),
  },
];

/**
 * The dataset an address asks for (`#dataset=statistical`), or the first one.
 * @param {string} hash  `location.hash`, with or without the leading `#`
 * @returns {DemoDataset}
 */
export function datasetFromHash(hash) {
  const id = new URLSearchParams(hash.replace(/^#/, "")).get("dataset");
  return DATASETS.find((d) => d.id === id) ?? DATASETS[0];
}
