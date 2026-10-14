// The contract between the app and a generator extension: a way to produce a new table from the
// loaded one (Tier 2 synthesis and Tier 3 sketching are the two that exist). The app knows only
// this shape, so a build can ship with any set of extensions, including none, without the app
// changing. The set is chosen at build time in `extensions.js`.

/** Bumped on any change to the shapes below that an existing extension would not survive. */
export const EXTENSION_API_VERSION = 1;

/**
 * What the app hands an extension: the loaded file and the user's edits to it. Read-only; an
 * extension never changes the original dataset.
 * @typedef {object} GeneratorHost
 * @property {Uint8Array | null} fileBytes
 * @property {string | null} fileExtension  the file's format, as the engine expects it
 * @property {string} fileName
 * @property {{ headers: string[], rowCount: number } | null} sample
 * @property {Set<number>} removedRowIndices
 * @property {Set<string>} removedColumns
 */

/**
 * A generated table: its rows, its profile and the run's report, plus whatever else the
 * extension keeps with it.
 * @typedef {{ sample: any, profile: any, report: any } & Record<string, any>} GeneratorResult
 */

/**
 * What an extension's hook returns. Members beyond these are the extension's own business,
 * passed back to its own `Dialog`.
 * @typedef {{
 *   result: GeneratorResult | null,
 *   clearResult: () => void,
 *   reset: () => void,
 *   downloadCsv: (format?: string) => void,
 *   downloadReport: () => void,
 * } & Record<string, any>} GeneratorState
 * `clearResult` forgets the generated table and keeps the settings; `reset` goes back to the
 * state of a newly loaded file.
 */

/**
 * Whether an extension can run in this setting. Absent means it can.
 * @typedef {{ state: "ready" } | { state: "locked", message: string }} ExtensionStatus
 */

/**
 * @typedef {object} GeneratorExtension
 * @property {number} apiVersion  must equal EXTENSION_API_VERSION
 * @property {string} id          dataset key; also the `data-kind` the styles color by
 * @property {2 | 3} tier         the tier picker entry that opens it
 * @property {string} label       dataset tab label, e.g. "Synthetic"
 * @property {string} noun        lower-case, for "Download synthetic data"
 * @property {string} emptyHint   tooltip on the dataset tab before anything was generated
 * @property {(host: GeneratorHost) => GeneratorState} useGenerator
 *   A React hook. The app calls it once per render, in list order.
 * @property {import("react").ComponentType<{ generator: GeneratorState, host: GeneratorHost,
 *   onClose: () => void, onDone: () => void }>} Dialog
 * @property {import("react").ComponentType<{ result: GeneratorResult }>} [Report]
 *   Shown in the analysis panel while the generated table is on screen.
 * @property {{ label: string, detail: (generator: GeneratorState) => string }} history
 *   The undo history entry written when a run finishes.
 * @property {(result: GeneratorResult | null) => Record<string, unknown>} [hostState]
 *   Extra keys reported to an embedding host (the Jupyter widget).
 * @property {(host: GeneratorHost) => ExtensionStatus} [status]
 */

/**
 * Checks a build's extension list once, when the app module loads, so a mismatch fails loudly
 * at startup instead of as a broken dialog later.
 * @param {GeneratorExtension[]} list
 * @returns {GeneratorExtension[]}
 */
export function checkExtensions(list) {
  const ids = new Set();
  const tiers = new Set();
  for (const ext of list) {
    if (ext.apiVersion !== EXTENSION_API_VERSION) {
      throw new Error(
        `Extension "${ext.id}" targets extension API ${ext.apiVersion}, this app provides ${EXTENSION_API_VERSION}`,
      );
    }
    if (ext.id === "original" || ids.has(ext.id)) {
      throw new Error(`Extension id "${ext.id}" is reserved or used twice`);
    }
    if (tiers.has(ext.tier)) {
      throw new Error(`Two extensions claim tier ${ext.tier}`);
    }
    ids.add(ext.id);
    tiers.add(ext.tier);
  }
  return list;
}

/**
 * Why the tier picker entry for a generator tier is disabled, or null when it can be used.
 * @param {GeneratorExtension | undefined} ext  the extension for that tier, if the build has one
 * @param {GeneratorHost} host
 * @returns {string | null}
 */
export function tierDisabledReason(ext, host) {
  if (!ext) return "Not available in this build";
  if (!host.sample) return "Load a file to generate data from it";
  const status = ext.status?.(host);
  if (status?.state === "locked") return status.message;
  return null;
}
