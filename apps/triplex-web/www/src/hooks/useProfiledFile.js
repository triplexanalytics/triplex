import { useCallback, useState } from "react";
import { profileFile } from "../state/profileClient";
import {
  buildContractFromProfile,
  createEmptyRecipe,
  upsertRecipeColumn,
} from "../state/yamlBuilder";
import { downloadBlob } from "../utils";

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
  const [error, setError] = useState("");

  const loadFile = useCallback(async (file) => {
    setError("");
    const buf = new Uint8Array(await file.arrayBuffer());
    const ext = file.name.split(".").pop().toLowerCase();
    setFileBytes(buf);
    setExtension(ext);
    try {
      const { sample: readSample, profile: readProfile } = await profileFile(buf, ext);
      setSample(readSample);
      setProfile(readProfile);
      setContract(buildContractFromProfile(readProfile));
      setRecipe(createEmptyRecipe());
      setSalt("");
    } catch (err) {
      setError(String(err.message || err));
    }
  }, []);

  const pickMethod = useCallback((columnName, method) => {
    setRecipe((r) => upsertRecipeColumn(r, columnName, method, "unused_in_the_browser"));
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
    salt,
    setSalt,
    error,
    loadFile,
    downloadContract,
    downloadRecipe,
  };
}
