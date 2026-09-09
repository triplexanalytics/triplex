import { useEffect, useRef, useState } from "react";
import { scoreDiversityMatrix } from "../state/datasetCardClient";

// Same fixed generalization width as the per-column k-anonymity advisory
// (useColumnRisk) — the Dataset Anonymity Analysis tab has no per-column
// bucket-width control the way the actual anonymize recipe does.
const RISK_BUCKET_WIDTH = 10;

/** l-diversity/t-closeness for every (integer column, candidate sensitive
 * column) pair. Computed in App.jsx as soon as sample/profile are ready —
 * same as useColumnRisk, and for the same reason: computing this only once
 * DiversityCheckSection mounts (i.e., only once the user opens that tab)
 * means the per-column k-anonymity bars (already resolved by then, since
 * useColumnRisk started as soon as the file loaded) visibly finish first,
 * with the matrix lagging a beat behind it instead of everything being
 * ready together by the time the user gets there. */
export function useDiversityMatrix(sample, profile) {
  const integerColumns = profile?.columns.filter((c) => c.inferred_type === "integer") ?? [];
  const allColumns = profile?.columns ?? [];

  const [raw, setRaw] = useState(null); // { lDiversity, tCloseness } | null
  const [loading, setLoading] = useState(false);
  const generation = useRef(0);

  useEffect(() => {
    if (!sample || !profile || integerColumns.length === 0) {
      setRaw(null);
      setLoading(false);
      return;
    }
    const myGeneration = ++generation.current;
    setLoading(true);
    const colIndex = {};
    sample.headers.forEach((name, i) => (colIndex[name] = i));
    // rows (quasi-identifier, bucketed) are always integer-inferred, so the
    // wasm call takes them as a typed array; cols (candidate sensitive
    // attribute) can be any column type, so those stay stringified cells.
    const rows = integerColumns.map((c) => ({
      name: c.name,
      values: sample.getNumericColumn(colIndex[c.name]),
    }));
    const cols = allColumns.map((c) => ({
      name: c.name,
      values: Array.from({ length: sample.rowCount }, (_, row) => sample.getCell(row, colIndex[c.name])),
    }));
    scoreDiversityMatrix(rows, cols, RISK_BUCKET_WIDTH)
      .then((result) => {
        if (generation.current !== myGeneration) return; // superseded by a newer upload
        setRaw(result);
        setLoading(false);
      })
      .catch(() => {
        if (generation.current !== myGeneration) return;
        setRaw(null);
        setLoading(false);
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sample, profile]);

  return { raw, loading, integerColumns, allColumns };
}
