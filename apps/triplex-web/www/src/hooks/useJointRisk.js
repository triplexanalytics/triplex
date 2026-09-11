import { useEffect, useRef, useState } from "react";
import { scoreJointRisk } from "../state/datasetCardClient";

// Same fixed generalization width as the per-column k-anonymity advisory
// (useColumnRisk) and the diversity matrix (useDiversityMatrix) — this is
// an advisory, not the actual anonymize recipe's bucket width.
const RISK_BUCKET_WIDTH = 10;

/** Combined quasi-identifier check (the design notes "dataset-level"
 * gap): scores `selectedNames` (2+ user-picked columns, by name) together
 * as one quasi-identifier instead of one column at a time, optionally
 * against `sensitiveName` for l-diversity/t-closeness. Returns `null`
 * until at least 2 columns are picked — a single column's joint score is
 * just that column's own score, already shown by `useColumnRisk`, so
 * there's nothing new to compute or show yet. */
export function useJointRisk(sample, profile, selectedNames, sensitiveName) {
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const generation = useRef(0);

  useEffect(() => {
    if (!sample || !profile || selectedNames.length < 2) {
      setResult(null);
      setLoading(false);
      return;
    }
    const myGeneration = ++generation.current;
    setLoading(true);
    const colIndex = {};
    sample.headers.forEach((name, i) => (colIndex[name] = i));
    const columnByName = {};
    profile.columns.forEach((c) => (columnByName[c.name] = c));
    const readColumn = (name) =>
      columnByName[name]?.inferred_type === "integer"
        ? { name, kind: "numeric", values: sample.getNumericColumn(colIndex[name]) }
        : {
            name,
            kind: "string",
            values: Array.from({ length: sample.rowCount }, (_, row) =>
              sample.getCell(row, colIndex[name]),
            ),
          };
    const columns = selectedNames.map(readColumn);
    const sensitiveValues = sensitiveName
      ? Array.from({ length: sample.rowCount }, (_, row) => sample.getCell(row, colIndex[sensitiveName]))
      : null;
    scoreJointRisk(columns, RISK_BUCKET_WIDTH, sensitiveValues)
      .then((r) => {
        if (generation.current !== myGeneration) return; // superseded by a newer selection/upload
        setResult(r);
        setLoading(false);
      })
      .catch(() => {
        if (generation.current !== myGeneration) return;
        setResult(null);
        setLoading(false);
      });
  }, [sample, profile, selectedNames, sensitiveName]);

  return { result, loading };
}
