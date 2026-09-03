import { useCallback, useState } from "react";
import { withColumnPattern } from "../state/yamlBuilder";
import { downloadBlob } from "../utils";

/** Owns the anonymize run and the pattern-conformance checks against its
 * output — both need `wasm` plus the file's bytes/contract/recipe, which is
 * why they're grouped in one hook separate from `useProfiledFile`.
 *
 * `salt` is a single value shared by every declared column, not a
 * per-column map: the design notes's cross-tier guarantee is that a given
 * source value maps to the same output value everywhere it recurs (the same
 * column across files, the same key across related tables) so joins on the
 * pseudonymized output still work. A different salt per column would break
 * that the moment the same real-world value showed up in two columns, for
 * no real security gain — HMAC's security doesn't degrade from reusing one
 * key across columns, it only changes the blast radius if that one salt
 * leaks, which isn't a trade-off this UI makes deliberately today. */
export function useAnonymize({ wasm, fileBytes, extension, recipe, salt, contract, setContract }) {
  const [anonymizeResult, setAnonymizeResult] = useState(null);
  const [patternChecks, setPatternChecks] = useState({});
  const [error, setError] = useState("");

  const run = useCallback(() => {
    if (!wasm || !fileBytes) return;
    setError("");
    try {
      const recipeYaml = wasm.recipe_json_to_yaml(JSON.stringify(recipe));
      const saltsJson = JSON.stringify(
        Object.fromEntries(recipe.columns.map((c) => [c.name, salt])),
      );
      const result = wasm.anonymize(fileBytes, extension, recipeYaml, saltsJson);
      setAnonymizeResult(result);
    } catch (err) {
      setError(String(err.message || err));
    }
  }, [wasm, fileBytes, extension, recipe, salt]);

  const checkPattern = useCallback(
    (columnName, pattern) => {
      if (!contract) return;
      const updated = withColumnPattern(contract, columnName, pattern);
      setContract(updated);
      if (!wasm || !pattern || !anonymizeResult) return;
      setError("");
      try {
        const contractYaml = wasm.contract_json_to_yaml(JSON.stringify(updated));
        const reportJson = wasm.check_pattern_conformance(
          anonymizeResult.masked_csv,
          extension,
          contractYaml,
        );
        setPatternChecks((p) => ({ ...p, [columnName]: JSON.parse(reportJson) }));
      } catch (err) {
        setError(String(err.message || err));
      }
    },
    [wasm, contract, setContract, anonymizeResult, extension],
  );

  const downloadData = useCallback(() => {
    if (!anonymizeResult) return;
    downloadBlob(anonymizeResult.masked_csv, "masked.csv", "text/csv");
  }, [anonymizeResult]);

  // Called by App.jsx right after a new file loads: this hook has no other
  // way to know the file underneath it changed, and a stale anonymizeResult/
  // patternChecks from the previous file would otherwise linger.
  const reset = useCallback(() => {
    setAnonymizeResult(null);
    setPatternChecks({});
    setError("");
  }, []);

  return { anonymizeResult, patternChecks, error, run, checkPattern, downloadData, reset };
}
