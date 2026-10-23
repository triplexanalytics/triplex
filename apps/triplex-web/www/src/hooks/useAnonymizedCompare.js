import { useEffect, useMemo, useState } from "react";
import { profileFile } from "../state/profileClient";
import { computeAssociation } from "../state/association";
import { rowsAlone, sharedQuasiIdentifiers } from "../state/sharedQuasiIdentifiers";
import { useColumnRisk } from "./useColumnRisk";
import { useDiversityMatrix } from "./useDiversityMatrix";

const NO_ROWS = new Set();
// Whole numbers are compared exactly: the original is measured as it is, and whatever ranges the
// recipe made show up as the anonymized side's own values.
const EXACT = 1;

/** The Compare view's "Original" and "Anonymized" sides. The anonymized side is the masked output
 * of the last anonymize run, profiled exactly as "Download data" would hand it over; columns the
 * run dropped appear on it as removed. Both sides' privacy matrices use the same quasi-identifiers
 * (`quasi.chosen`, which the user can change), so the risk before and after reads off directly.
 *
 * The masked output is profiled as soon as a run finishes, so Compare can open at once; the
 * scoring waits until `active` (the Compare view is open on this pair). A new run replaces the
 * side; a cleared result (another file loaded) removes it. */
export function useAnonymizedCompare({ file, original, anonymizeResult, stale, active }) {
  const [masked, setMasked] = useState(null); // { sample, profile } | null
  const [error, setError] = useState("");
  useEffect(() => {
    setMasked(null);
    setError("");
    if (!anonymizeResult) return;
    let current = true;
    profileFile(anonymizeResult.masked_csv, "csv")
      .then((read) => current && setMasked(read))
      .catch((err) => current && setError(String(err?.message || err)));
    return () => {
      current = false;
    };
  }, [anonymizeResult]);

  const included = file.sample ? file.sample.rowCount - file.removedRowIndices.size : 0;
  const defaults = useMemo(
    () =>
      file.profile && masked
        ? sharedQuasiIdentifiers(file.profile, masked.profile, included)
        : { candidates: [], chosen: [] },
    [file.profile, masked, included],
  );
  const [chosen, setChosen] = useState([]);
  useEffect(() => setChosen(defaults.chosen), [defaults]);
  const toggle = (name) =>
    setChosen((c) =>
      c.includes(name)
        ? c.filter((n) => n !== name)
        : defaults.candidates.filter((n) => n === name || c.includes(n)),
    );

  const scoring = active && !!masked;
  const options = { quasiNames: chosen, bucketWidth: EXACT };
  const leftMatrix = useDiversityMatrix(
    scoring ? file.sample : null,
    file.profile,
    file.removedRowIndices,
    options,
  );
  const rightMatrix = useDiversityMatrix(
    scoring ? masked.sample : null,
    masked?.profile,
    NO_ROWS,
    options,
  );
  const rightRisk = useColumnRisk(scoring ? masked.sample : null, masked?.profile, NO_ROWS);
  const rightAssociation = useMemo(
    () => (scoring ? computeAssociation(masked.sample, masked.profile, NO_ROWS) : null),
    [scoring, masked],
  );

  const summary = useMemo(
    () =>
      scoring && chosen.length > 0
        ? {
            left: rowsAlone(file.sample, chosen, file.removedRowIndices),
            right: rowsAlone(masked.sample, chosen, NO_ROWS),
          }
        : null,
    [scoring, chosen, file.sample, file.removedRowIndices, masked],
  );

  const droppedReasons = useMemo(() => {
    const dropped = anonymizeResult ? JSON.parse(anonymizeResult.report_json).columns_dropped : [];
    return Object.fromEntries((dropped ?? []).map((name) => [name, "the recipe drops it"]));
  }, [anonymizeResult]);

  if (!masked || !file.sample || !file.profile) return { ready: false, error };
  return {
    ready: true,
    error,
    // Every original column, the dropped ones included, so the anonymized side can show them as
    // removed instead of leaving them out without a trace.
    left: {
      label: "Original",
      kind: "original",
      sample: file.sample,
      profile: file.profile,
      analysis: { ...original, diversityMatrix: leftMatrix },
      excluded: file.removedRowIndices,
    },
    right: {
      label: "Anonymized",
      kind: "anonymized",
      sample: masked.sample,
      profile: masked.profile,
      analysis: {
        columnRisk: rightRisk,
        diversityMatrix: rightMatrix,
        association: rightAssociation,
        columnSubsets: undefined,
      },
      excluded: NO_ROWS,
      droppedReasons,
    },
    quasi: { candidates: defaults.candidates, chosen, toggle, summary },
    stale,
  };
}
