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
 * parameter rather than owned here. */
export function useProfiledFile(wasm) {
  const [fileBytes, setFileBytes] = useState(null);
  const [extension, setExtension] = useState("");
  const [sample, setSample] = useState(null);
  const [profile, setProfile] = useState(null);
  const [contract, setContract] = useState(null);
  const [recipe, setRecipe] = useState(createEmptyRecipe());
  const [salt, setSalt] = useState("");
  // Columns the user removed from the working view — a frontend-only hide
  // (drops it from the table, contract, and recipe), not a backend "drop
  // column" action: the anonymize/export pipeline has no such action yet,
  // so "Download data" still includes it in the masked CSV until that lands.
  const [removedColumns, setRemovedColumns] = useState(() => new Set());
  const [error, setError] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [loadingFileName, setLoadingFileName] = useState("");

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
      timingLog(`[main] total loadFile: ${(performance.now() - t0).toFixed(1)}ms`);
    } catch (err) {
      setError(String(err.message || err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const pickMethod = useCallback((columnName, method) => {
    setRecipe((r) => upsertRecipeColumn(r, columnName, method, "unused_in_the_browser"));
  }, []);

  const removeColumn = useCallback((columnName) => {
    setRemovedColumns((s) => new Set(s).add(columnName));
    setRecipe((r) => removeRecipeColumn(r, columnName));
    setContract((c) => c && { ...c, columns: c.columns.filter((col) => col.name !== columnName) });
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
    pickMethod,
    removedColumns,
    removeColumn,
    salt,
    setSalt,
    error,
    isLoading,
    loadingFileName,
    loadFile,
    downloadContract,
    downloadRecipe,
  };
}
