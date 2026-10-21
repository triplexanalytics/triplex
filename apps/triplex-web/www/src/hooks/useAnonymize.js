import { useCallback, useState } from "react";
import { withColumnPattern } from "../state/yamlBuilder";
import { downloadBlob, mimeForExtension } from "../utils";
import { runAnonymize, checkPatternConformance, convertBytes } from "../state/anonymizeClient";

/** Owns the anonymize run and the pattern-conformance checks against its
 * output — both need `wasm` plus the file's bytes/contract/recipe, which is
 * why they're grouped in one hook separate from `useProfiledFile`.
 *
 * `salt` is a single value shared by every declared column, not a
 * per-column map: the cross-tier guarantee is that a given
 * source value maps to the same output value everywhere it recurs (the same
 * column across files, the same key across related tables) so joins on the
 * pseudonymized output still work. A different salt per column would break
 * that the moment the same real-world value showed up in two columns, for
 * no real security gain — HMAC's security doesn't degrade from reusing one
 * key across columns, it only changes the blast radius if that one salt
 * leaks, which isn't a trade-off this UI makes deliberately today.
 *
 * The actual masking (`anonymize`) and full-file pattern re-scan
 * (`check_pattern_conformance`) run in a worker (anonymizeClient) since
 * both touch every row of the dataset — `recipe_json_to_yaml`/
 * `contract_json_to_yaml` stay on the main thread here since they only
 * touch the (column-count-sized, not row-count-sized) recipe/contract.
 *
 * `removedRowIndices` (`useProfiledFile.js`'s own state, "remove these rows
 * from the output") is forwarded straight to `wasm.anonymize`'s
 * `excluded_row_indices` — those rows never enter the masked output at all,
 * not just hidden from the Table tab (this
 * belongs to the real anonymize call, not a client-side pre-filter). */
export function useAnonymize({
  wasm,
  fileBytes,
  extension,
  recipe,
  salt,
  contract,
  setContract,
  removedRowIndices,
}) {
  const [anonymizeResult, setAnonymizeResult] = useState(null);
  const [anonymizing, setAnonymizing] = useState(false);
  const [patternChecks, setPatternChecks] = useState({});
  const [checkingColumns, setCheckingColumns] = useState(() => new Set());
  const [error, setError] = useState("");

  const run = useCallback(async () => {
    if (!wasm || !fileBytes) return;
    setError("");
    setAnonymizing(true);
    try {
      const recipeYaml = wasm.recipe_json_to_yaml(JSON.stringify(recipe));
      const saltsJson = JSON.stringify(
        Object.fromEntries(recipe.columns.map((c) => [c.name, salt])),
      );
      const result = await runAnonymize(
        fileBytes,
        extension,
        recipeYaml,
        saltsJson,
        Array.from(removedRowIndices ?? []),
      );
      setAnonymizeResult(result);
    } catch (err) {
      setError(String(err.message || err));
    } finally {
      setAnonymizing(false);
    }
  }, [wasm, fileBytes, extension, recipe, salt, removedRowIndices]);

  const checkPattern = useCallback(
    async (columnName, pattern) => {
      if (!contract) return;
      const updated = withColumnPattern(contract, columnName, pattern);
      setContract(updated);
      if (!wasm || !pattern || !anonymizeResult) return;
      setError("");
      setCheckingColumns((s) => new Set(s).add(columnName));
      try {
        const contractYaml = wasm.contract_json_to_yaml(JSON.stringify(updated));
        const reportJson = await checkPatternConformance(
          anonymizeResult.masked_csv,
          extension,
          contractYaml,
        );
        setPatternChecks((p) => ({ ...p, [columnName]: JSON.parse(reportJson) }));
      } catch (err) {
        setError(String(err.message || err));
      } finally {
        setCheckingColumns((s) => {
          const next = new Set(s);
          next.delete(columnName);
          return next;
        });
      }
    },
    [wasm, contract, setContract, anonymizeResult, extension],
  );

  // `format` is the extension the user picked in the download-format
  // picker, or falsy to keep whatever the source is already in. The source
  // is either the anonymize run's output (always CSV -- see
  // `triplex_anonymizer::anonymize_bytes`) or, if anonymize hasn't run
  // yet, the original uploaded bytes as-is: "no transformation applied"
  // shouldn't block downloading the data, it just downloads the input
  // unchanged (in whichever format the picker asks for).
  const downloadData = useCallback(
    async (format) => {
      const sourceBytes = anonymizeResult ? anonymizeResult.masked_csv : fileBytes;
      const sourceExtension = anonymizeResult ? "csv" : extension;
      if (!sourceBytes) return;
      const targetExtension = format || sourceExtension;
      const baseName = anonymizeResult ? "masked" : "original";
      try {
        const bytes =
          targetExtension === sourceExtension
            ? sourceBytes
            : await convertBytes(sourceBytes, sourceExtension, targetExtension);
        downloadBlob(bytes, `${baseName}.${targetExtension}`, mimeForExtension(targetExtension));
      } catch (err) {
        setError(String(err.message || err));
      }
    },
    [anonymizeResult, fileBytes, extension],
  );

  // The mapping file reverses the pseudonyms: it is offered only after a run, as a file of its own,
  // so it can be kept apart from the masked data it unlocks.
  const downloadMapping = useCallback(() => {
    if (anonymizeResult?.mapping_csv) {
      downloadBlob(anonymizeResult.mapping_csv, "mapping.csv", "text/csv");
    }
  }, [anonymizeResult]);

  // Called by App.jsx right after a new file loads: this hook has no other
  // way to know the file underneath it changed, and a stale anonymizeResult/
  // patternChecks from the previous file would otherwise linger.
  const reset = useCallback(() => {
    setAnonymizeResult(null);
    setPatternChecks({});
    setCheckingColumns(new Set());
    setError("");
  }, []);

  return {
    anonymizeResult,
    anonymizing,
    patternChecks,
    checkingColumns,
    error,
    run,
    checkPattern,
    downloadData,
    downloadMapping,
    reset,
  };
}
