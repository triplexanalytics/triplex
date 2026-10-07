import { useCallback, useMemo, useState } from "react";
import { downloadBlob } from "../utils";
import { profileFile } from "../state/profileClient";
import {
  runSynthesize,
  runSynthesizeBayes,
  runSynthesizeDp,
  suggestDomains,
} from "../state/synthesisClient";
import {
  DEFAULT_BINS,
  DEFAULT_MARGINAL_SHARE,
  DEFAULT_MIN_CATEGORY_COUNT,
  DEFAULT_MAX_PARENTS,
  DEFAULT_METHOD,
  DEFAULT_STRUCTURE,
  DEFAULT_STRUCTURE_SHARE,
  INDEPENDENT_STRUCTURE,
  isBinned,
  DP_DIAGNOSTICS_DEFAULT,
  applySuggestions,
  blankColumns,
  buildDpConfig,
  rowsInUse,
  updateColumn,
  validateBayes,
  validateCommon,
  validateDp,
} from "../state/synthesisConfig";

/** Owns the Tier 2 (synthetic data) form, run and result. Lives in App.jsx beside useAnonymize
 * (not inside the dialog) so the result and the form survive closing the dialog.
 *
 * The file's bytes never leave the browser: every call goes to synthesize.worker.js. Rows and
 * columns the user removed in the table are forwarded to the run, so they
 * cannot influence the synthetic output.
 *
 * Privacy-relevant behavior lives in synthesisConfig.js (unit-tested): a DP run needs an epsilon
 * (no default) and PUBLIC bounds, and bounds suggested from the data block the run until the
 * user confirms they are public knowledge. */
export function useSynthesize({
  fileBytes,
  extension,
  headers,
  totalRows,
  removedRowIndices,
  removedColumns,
}) {
  const [mode, setMode] = useState("standard");
  const [seed, setSeed] = useState("42");
  const [rowsOut, setRowsOut] = useState("");
  const [minCategoryCount, setMinCategoryCount] = useState(String(DEFAULT_MIN_CATEGORY_COUNT));
  const [epsilon, setEpsilon] = useState("");
  const [marginalShare, setMarginalShare] = useState(String(DEFAULT_MARGINAL_SHARE));
  const [bins, setBins] = useState(String(DEFAULT_BINS));
  // Which model: the Gaussian copula ("copula") or a Bayesian network ("bayesnet"), whose
  // structure is a tree ("chow_liu") or up to `maxParents` parents per column ("k_parent").
  const [method, setMethod] = useState(DEFAULT_METHOD);
  const [structure, setStructure] = useState(DEFAULT_STRUCTURE);
  const [maxParents, setMaxParents] = useState(String(DEFAULT_MAX_PARENTS));
  const [structureShare, setStructureShare] = useState(String(DEFAULT_STRUCTURE_SHARE));
  const [columnEdits, setColumnEdits] = useState(null);
  const [confirmedPublic, setConfirmedPublic] = useState(false);
  const [includeDiagnostics, setIncludeDiagnostics] = useState(DP_DIAGNOSTICS_DEFAULT);
  const [result, setResult] = useState(null);
  const [running, setRunning] = useState(false);
  const [suggesting, setSuggesting] = useState(false);
  const [error, setError] = useState("");

  const columns = useMemo(
    () => columnEdits ?? blankColumns(headers ?? [], removedColumns),
    [columnEdits, headers, removedColumns],
  );
  const rowCount = rowsInUse(totalRows ?? 0, removedRowIndices?.size ?? 0);

  const validation = useMemo(() => {
    const common = validateCommon({ seed, rows: rowsOut });
    if (mode !== "dp") {
      const network = isBinned(method)
        ? validateBayes({
            structure: method === "marginals" ? INDEPENDENT_STRUCTURE : structure,
            maxParents,
            bins,
          })
        : [];
      const errors = [...common, ...network];
      return { ok: errors.length === 0, errors };
    }
    const dp = validateDp({
      epsilon,
      marginalShare,
      bins,
      columns,
      confirmedPublic,
      rowCount,
      method,
      structure,
      maxParents,
      structureShare,
    });
    const errors = [...common, ...dp.errors];
    return { ok: errors.length === 0, errors };
  }, [
    mode,
    seed,
    rowsOut,
    epsilon,
    marginalShare,
    bins,
    columns,
    confirmedPublic,
    rowCount,
    method,
    structure,
    maxParents,
    structureShare,
  ]);

  const editColumn = useCallback(
    (name, patch) => {
      setColumnEdits((current) =>
        updateColumn(current ?? blankColumns(headers ?? [], removedColumns), name, patch),
      );
      // Editing a bound makes it the user's own declaration; a fresh confirmation is only needed
      // for values that are still suggestions, which validateDp re-derives on its own.
    },
    [headers, removedColumns],
  );

  const suggest = useCallback(async () => {
    if (!fileBytes) return;
    setError("");
    setSuggesting(true);
    try {
      const suggestions = JSON.parse(await suggestDomains(fileBytes, extension));
      setColumnEdits((current) =>
        applySuggestions(
          current ?? blankColumns(headers ?? [], removedColumns),
          suggestions,
          removedColumns,
        ),
      );
      setConfirmedPublic(false);
    } catch (e) {
      setError(String(e.message ?? e));
    } finally {
      setSuggesting(false);
    }
  }, [fileBytes, extension, headers, removedColumns]);

  const run = useCallback(async () => {
    if (!fileBytes || !validation.ok) return false;
    setError("");
    setRunning(true);
    try {
      const common = {
        bytes: fileBytes,
        extension,
        seed: Number(seed) >>> 0,
        rows: rowsOut.trim() === "" ? 0 : Number(rowsOut),
        excludedRowIndices: [...(removedRowIndices ?? [])],
      };
      const output =
        mode === "dp"
          ? await runSynthesizeDp({
              ...common,
              config: buildDpConfig({
                epsilon,
                marginalShare,
                bins,
                columns,
                method,
                structure,
                maxParents,
                structureShare,
              }),
              withDiagnostics: includeDiagnostics,
            })
          : isBinned(method)
            ? await runSynthesizeBayes({
                ...common,
                minCategoryCount: Number(minCategoryCount),
                excludedColumns: [...(removedColumns ?? [])],
                structure: method === "marginals" ? INDEPENDENT_STRUCTURE : structure,
                maxParents: Number(maxParents) || DEFAULT_MAX_PARENTS,
                bins: Number(bins),
              })
            : await runSynthesize({
                ...common,
                minCategoryCount: Number(minCategoryCount),
                excludedColumns: [...(removedColumns ?? [])],
              });
      // The synthetic table goes through the same profiling path as an uploaded file, so the
      // Table and the side panel can show it like any other dataset.
      const { sample, profile } = await profileFile(output.synthetic_csv, "csv");
      setResult({
        mode,
        csv: output.synthetic_csv,
        reportJson: output.report_json,
        report: JSON.parse(output.report_json),
        diagnosticsIncluded: mode === "dp" && includeDiagnostics,
        sample,
        profile,
      });
      return true;
    } catch (e) {
      setError(String(e.message ?? e));
      return false;
    } finally {
      setRunning(false);
    }
  }, [
    fileBytes,
    validation.ok,
    extension,
    seed,
    rowsOut,
    removedRowIndices,
    removedColumns,
    mode,
    epsilon,
    marginalShare,
    bins,
    columns,
    includeDiagnostics,
    minCategoryCount,
    method,
    structure,
    maxParents,
    structureShare,
  ]);

  const downloadCsv = useCallback(() => {
    if (result) downloadBlob(result.csv, "synthetic.csv", "text/csv");
  }, [result]);

  const downloadReport = useCallback(() => {
    if (result) downloadBlob(result.reportJson, "synthesis_report.json", "application/json");
  }, [result]);

  const clearResult = useCallback(() => setResult(null), []);

  const reset = useCallback(() => {
    setColumnEdits(null);
    setConfirmedPublic(false);
    setIncludeDiagnostics(DP_DIAGNOSTICS_DEFAULT);
    setEpsilon("");
    setResult(null);
    setError("");
  }, []);

  return {
    mode,
    setMode,
    seed,
    setSeed,
    rowsOut,
    setRowsOut,
    minCategoryCount,
    setMinCategoryCount,
    epsilon,
    setEpsilon,
    marginalShare,
    setMarginalShare,
    bins,
    setBins,
    method,
    setMethod,
    structure,
    setStructure,
    maxParents,
    setMaxParents,
    structureShare,
    setStructureShare,
    columns,
    editColumn,
    confirmedPublic,
    setConfirmedPublic,
    includeDiagnostics,
    setIncludeDiagnostics,
    rowCount,
    validation,
    suggest,
    suggesting,
    run,
    running,
    result,
    error,
    downloadCsv,
    downloadReport,
    clearResult,
    reset,
  };
}
