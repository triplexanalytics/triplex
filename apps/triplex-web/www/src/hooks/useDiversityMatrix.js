import { useEffect, useRef, useState } from "react";
import { scoreDiversityMatrix } from "../state/datasetCardClient";

// Same fixed generalization width as the per-column k-anonymity advisory
// (useColumnRisk) — the Dataset Anonymity Analysis tab has no per-column
// bucket-width control the way the actual anonymize recipe does.
const RISK_BUCKET_WIDTH = 10;

/** l-diversity/t-closeness for every (quasi-identifier column, candidate
 * sensitive column) pair, plus the combined-k-anonymity matrix (every
 * quasi-identifier pair's *joint* k-anonymity — the dataset-level verdict).
 * A quasi-identifier is an integer column (generalized into a numeric
 * bucket) or a categorical/string column (scored on its exact values,
 * the design notes). Computed in App.jsx as
 * soon as sample/profile are ready — same as useColumnRisk, and for the
 * same reason: computing this only once DiversityCheckSection mounts (i.e.,
 * only once the user opens that tab) means the per-column k-anonymity bars
 * (already resolved by then, since useColumnRisk started as soon as the
 * file loaded) visibly finish first, with the matrix lagging a beat behind
 * it instead of everything being ready together by the time the user gets
 * there. */
export function useDiversityMatrix(sample, profile) {
  const quasiColumns =
    profile?.columns.filter((c) => c.inferred_type === "integer" || c.distribution?.kind === "categorical") ?? [];
  const allColumns = profile?.columns ?? [];

  const [raw, setRaw] = useState(null); // { lDiversity, tCloseness, betaLikeness, deltaDisclosure, jointK, rowRisk } | null
  const [loading, setLoading] = useState(false);
  const generation = useRef(0);

  useEffect(() => {
    if (!sample || !profile || quasiColumns.length === 0) {
      setRaw(null);
      setLoading(false);
      return;
    }
    const myGeneration = ++generation.current;
    setLoading(true);
    const colIndex = {};
    sample.headers.forEach((name, i) => (colIndex[name] = i));
    // rows (quasi-identifier): an integer column's wasm call takes a typed
    // array; a categorical/string column stays stringified cells, same as
    // cols (candidate sensitive attribute, which can be any column type).
    const rows = quasiColumns.map((c) =>
      c.inferred_type === "integer"
        ? { name: c.name, kind: "numeric", values: sample.getNumericColumn(colIndex[c.name]) }
        : {
            name: c.name,
            kind: "string",
            values: Array.from({ length: sample.rowCount }, (_, row) =>
              sample.getCell(row, colIndex[c.name]),
            ),
          },
    );
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

  return { raw, loading, quasiColumns, allColumns };
}
