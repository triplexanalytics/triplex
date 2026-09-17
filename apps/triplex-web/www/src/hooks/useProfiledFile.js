import { useCallback, useState } from "react";
import { profileFile } from "../state/profileClient";
import {
  buildContractFromProfile,
  createEmptyRecipe,
  upsertRecipeColumn,
  removeRecipeColumn,
} from "../state/yamlBuilder";
import { downloadBlob } from "../utils";
import { timingLog } from "../state/wasmTiming";

/** Owns the uploaded file's lifecycle: raw bytes, the worker-computed
 * sample/profile, the contract/recipe built up from table interactions, and
 * the single salt shared by every column (see `useAnonymize`'s doc comment
 * for why one salt, not one per column). Exposes the raw `setContract`
 * rather than a pattern-specific setter: `useAnonymize.checkPattern` needs
 * the freshly-updated contract value synchronously (to build the YAML for
 * an immediate conformance check), not just to schedule a state update, so
 * it computes the update itself and calls this setter directly. Doesn't
 * need `wasm` itself for that: `profileFile` runs in a Web Worker with its
 * own wasm instance, and `buildContractFromProfile` is plain JS. Only the
 * two downloads below call into the main-thread wasm module, taken as a
 * parameter rather than owned here.
 *
 * `removedRowIndices` is the row analog of `removedColumns` -- see its own
 * comment below and `removeRows`/`restoreRemovedRows`. */
export function useProfiledFile(wasm) {
  const [fileBytes, setFileBytes] = useState(null);
  const [extension, setExtension] = useState("");
  const [sample, setSample] = useState(null);
  const [profile, setProfile] = useState(null);
  const [contract, setContract] = useState(null);
  const [recipe, setRecipe] = useState(createEmptyRecipe());
  const [salt, setSalt] = useState("");
  // Columns currently hidden from the table view -- exactly the columns
  // whose recipe method is "drop" ("Delete from output," see pickMethod
  // below): one merged action, not two. It used to be its own independent
  // "Remove from dataset" button that hid a column from the table/contract
  // without the anonymize/export pipeline actually excluding it from the
  // output (a real gap: "Download data" still included it) -- now that
  // AnonymizeMethod::Drop genuinely excludes it, keeping a second,
  // differently-behaved hide action alongside it was just confusing (two
  // buttons that each hide the column, doing different things to it).
  const [removedColumns, setRemovedColumns] = useState(() => new Set());
  // Rows removed from the output entirely -- the row analog of
  // removedColumns/AnonymizeMethod::Drop above, driven by TableTab.jsx's
  // row-filter banner ("Remove these rows") rather than a per-column
  // method. Threaded into useAnonymize as excluded_row_indices
  //  -- like Drop, this only takes effect through an actual anonymize
  // run, not the "download original before anonymizing" bypass.
  const [removedRowIndices, setRemovedRowIndices] = useState(() => new Set());
  const [error, setError] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [loadingFileName, setLoadingFileName] = useState("");
  const [fileName, setFileName] = useState("");

  const loadFile = useCallback(async (file) => {
    setError("");
    setIsLoading(true);
    setLoadingFileName(file.name);
    try {
      const t0 = performance.now();
      const buf = new Uint8Array(await file.arrayBuffer());
      const t1 = performance.now();
      const ext = file.name.split(".").pop().toLowerCase();
      setFileBytes(buf);
      setExtension(ext);
      timingLog(`[main] file.arrayBuffer(): ${(t1 - t0).toFixed(1)}ms for ${buf.length} bytes`);
      const { sample: readSample, profile: readProfile } = await profileFile(buf, ext);
      const t2 = performance.now();
      timingLog(`[main] profileFile round trip (postMessage + worker + parse): ${(t2 - t1).toFixed(1)}ms`);
      setSample(readSample);
      setProfile(readProfile);
      setContract(buildContractFromProfile(readProfile));
      setRecipe(createEmptyRecipe());
      setSalt("");
      setRemovedColumns(new Set());
      setRemovedRowIndices(new Set());
      setFileName(file.name);
      timingLog(`[main] total loadFile: ${(performance.now() - t0).toFixed(1)}ms`);
      return readSample;
    } catch (err) {
      setError(String(err.message || err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Picking "drop" hides the column from the table view too (see
  // removedColumns above) -- a dropped column can't also be half-shown, so
  // this is the one place that toggles both together. Picking any other
  // method after a drop implicitly undoes the hide: a column can't be both
  // hidden-as-dropped and actively masked with something else, so choosing
  // a real method is itself the "no, mask it instead" signal, without
  // requiring a separate undo click first.
  const pickMethod = useCallback((columnName, method) => {
    setRecipe((r) => upsertRecipeColumn(r, columnName, method, "unused_in_the_browser"));
    setRemovedColumns((s) => {
      const isDropped = s.has(columnName);
      if (method === "drop") return isDropped ? s : new Set(s).add(columnName);
      if (!isDropped) return s;
      const next = new Set(s);
      next.delete(columnName);
      return next;
    });
  }, []);

  // Undoes "Delete from output": un-declares the recipe entry and restores
  // table visibility together, the same pairing pickMethod sets up.
  const clearMethod = useCallback((columnName) => {
    setRecipe((r) => removeRecipeColumn(r, columnName));
    setRemovedColumns((s) => {
      if (!s.has(columnName)) return s;
      const next = new Set(s);
      next.delete(columnName);
      return next;
    });
  }, []);

  // Adds `rowIndices` (any iterable, typically App.jsx's current
  // rowFilter.filteredRowIndices) to the removed set -- additive, so
  // removing a second selection doesn't undo the first.
  const removeRows = useCallback((rowIndices) => {
    setRemovedRowIndices((s) => {
      const next = new Set(s);
      for (const i of rowIndices) next.add(i);
      return next;
    });
  }, []);

  const restoreRemovedRows = useCallback(() => {
    setRemovedRowIndices(new Set());
  }, []);

  // Back to the pre-upload state -- the History tab's undo for the "Load
  // dataset" entry (see App.jsx), since there's no earlier file state to
  // restore to.
  const reset = useCallback(() => {
    setFileBytes(null);
    setExtension("");
    setSample(null);
    setProfile(null);
    setContract(null);
    setRecipe(createEmptyRecipe());
    setSalt("");
    setRemovedColumns(new Set());
    setRemovedRowIndices(new Set());
    setError("");
    setFileName("");
  }, []);

  const downloadContract = useCallback(() => {
    if (!wasm || !contract) return;
    downloadBlob(
      wasm.contract_json_to_yaml(JSON.stringify(contract)),
      "contract.triplex.yaml",
      "text/yaml",
    );
  }, [wasm, contract]);

  const downloadRecipe = useCallback(() => {
    if (!wasm) return;
    downloadBlob(
      wasm.recipe_json_to_yaml(JSON.stringify(recipe)),
      "recipe.triplex.yaml",
      "text/yaml",
    );
  }, [wasm, recipe]);

  return {
    fileBytes,
    extension,
    sample,
    profile,
    contract,
    setContract,
    recipe,
    setRecipe,
    pickMethod,
    clearMethod,
    removedColumns,
    setRemovedColumns,
    removedRowIndices,
    setRemovedRowIndices,
    removeRows,
    restoreRemovedRows,
    reset,
    salt,
    setSalt,
    error,
    isLoading,
    loadingFileName,
    fileName,
    loadFile,
    downloadContract,
    downloadRecipe,
  };
}
