import { useCallback, useMemo, useState } from "react";
import { downloadBlob } from "../utils";
import { profileFile } from "../state/profileClient";
import { mergeSketchClasses, proposeSketch, runSynthesizeSketch } from "../state/synthesisClient";
import { DEFAULT_MIN_CATEGORY_COUNT, validateCommon } from "../state/synthesisConfig";
import {
  DEFAULT_CLASSES,
  DEFAULT_MIN_CLASS_SIZE,
  DEFAULT_SKETCH_BINS,
  dropClass,
  setAxisRange,
  setClassSize,
  sketchProblem,
  validateSketchSettings,
} from "../state/sketchEdit";

/** Owns the Tier 3 sketch: the proposal settings, the sketch as proposed and
 * steered, the run and its result. Lives in App.jsx beside useSynthesize so the sketch survives
 * closing the dialog.
 *
 * Nothing leaves the browser: proposing, merging and generating all go to synthesize.worker.js.
 * The sketch is derived from the data (class ranges end at real values), so it is kept in page
 * state like the data and is only ever downloaded by the user. No privacy guarantee is made. */
export function useSketch({ fileBytes, extension, removedRowIndices, removedColumns }) {
  const [classes, setClasses] = useState(String(DEFAULT_CLASSES));
  const [minClassSize, setMinClassSize] = useState(String(DEFAULT_MIN_CLASS_SIZE));
  const [bins, setBins] = useState(String(DEFAULT_SKETCH_BINS));
  const [seed, setSeed] = useState("42");
  const [rowsOut, setRowsOut] = useState("");
  const [sketch, setSketch] = useState(null);
  const [selectedClass, setSelectedClass] = useState(0);
  const [result, setResult] = useState(null);
  const [busy, setBusy] = useState(null); // "proposing" | "merging" | "generating" | null
  const [error, setError] = useState("");

  const settingErrors = useMemo(
    () => [
      ...validateCommon({ seed, rows: rowsOut }),
      ...validateSketchSettings({ classes, minClassSize, bins }),
    ],
    [seed, rowsOut, classes, minClassSize, bins],
  );
  const problem = sketchProblem(sketch);

  const common = useCallback(
    () => ({
      bytes: fileBytes,
      extension,
      seed: Number(seed) >>> 0,
      minCategoryCount: DEFAULT_MIN_CATEGORY_COUNT,
      excludedRowIndices: [...(removedRowIndices ?? [])],
      excludedColumns: [...(removedColumns ?? [])],
    }),
    [fileBytes, extension, seed, removedRowIndices, removedColumns],
  );

  const attempt = useCallback(async (what, work) => {
    setError("");
    setBusy(what);
    try {
      await work();
      return true;
    } catch (e) {
      setError(String(e.message ?? e));
      return false;
    } finally {
      setBusy(null);
    }
  }, []);

  const propose = useCallback(() => {
    if (!fileBytes || settingErrors.length > 0) return false;
    return attempt("proposing", async () => {
      const json = await proposeSketch({
        ...common(),
        classes: Number(classes),
        minClassSize: Number(minClassSize),
        bins: Number(bins),
      });
      setSketch(JSON.parse(json));
      setSelectedClass(0);
    });
  }, [fileBytes, settingErrors, attempt, common, classes, minClassSize, bins]);

  const edit = useCallback((change) => setSketch((s) => (s ? change(s) : s)), []);
  const moveRange = useCallback(
    (classIndex, columnIndex, range) =>
      edit((s) => setAxisRange(s, classIndex, columnIndex, range)),
    [edit],
  );
  const resizeClass = useCallback(
    (classIndex, size) => edit((s) => setClassSize(s, classIndex, size)),
    [edit],
  );
  const removeClass = useCallback(
    (classIndex) => {
      edit((s) => dropClass(s, classIndex));
      setSelectedClass(0);
    },
    [edit],
  );
  const merge = useCallback(
    (a, b) => {
      if (!sketch) return false;
      return attempt("merging", async () => {
        setSketch(JSON.parse(await mergeSketchClasses(sketch, a, b)));
        setSelectedClass(Math.min(a, b));
      });
    },
    [sketch, attempt],
  );

  const generate = useCallback(() => {
    if (!fileBytes || !sketch || problem || settingErrors.length > 0) return false;
    return attempt("generating", async () => {
      const output = await runSynthesizeSketch({
        ...common(),
        sketch,
        rows: rowsOut.trim() === "" ? 0 : Number(rowsOut),
      });
      // Profiled like an uploaded file, so the Table and the side panel can show it.
      const { sample, profile } = await profileFile(output.synthetic_csv, "csv");
      setResult({
        mode: "standard",
        csv: output.synthetic_csv,
        reportJson: output.report_json,
        report: JSON.parse(output.report_json),
        diagnosticsIncluded: false,
        sample,
        profile,
      });
    });
  }, [fileBytes, sketch, problem, settingErrors, attempt, common, rowsOut]);

  const downloadCsv = useCallback(() => {
    if (result) downloadBlob(result.csv, "sketched.csv", "text/csv");
  }, [result]);
  const downloadReport = useCallback(() => {
    if (result) downloadBlob(result.reportJson, "synthesis_report.json", "application/json");
  }, [result]);
  const downloadSketch = useCallback(() => {
    if (sketch) downloadBlob(JSON.stringify(sketch, null, 2), "sketch.json", "application/json");
  }, [sketch]);

  const clearResult = useCallback(() => setResult(null), []);
  const reset = useCallback(() => {
    setSketch(null);
    setSelectedClass(0);
    setResult(null);
    setError("");
  }, []);

  return {
    classes,
    setClasses,
    minClassSize,
    setMinClassSize,
    bins,
    setBins,
    seed,
    setSeed,
    rowsOut,
    setRowsOut,
    settingErrors,
    sketch,
    selectedClass,
    setSelectedClass,
    problem,
    propose,
    moveRange,
    resizeClass,
    removeClass,
    merge,
    generate,
    busy,
    result,
    error,
    downloadCsv,
    downloadReport,
    downloadSketch,
    clearResult,
    reset,
  };
}
