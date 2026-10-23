import { useEffect, useRef, useState } from "react";
import { scoreDiversityMatrix } from "../state/datasetCardClient";
import { isQuasiIdentifierKind } from "../state/sharedQuasiIdentifiers";

// Same fixed generalization width as the per-column k-anonymity advisory
// (useColumnRisk) — the Dataset Anonymity Analysis tab has no per-column
// bucket-width control the way the actual anonymize recipe does.
const RISK_BUCKET_WIDTH = 10;

/** l-diversity/t-closeness for every (quasi-identifier column, candidate
 * sensitive column) pair, plus the combined-k-anonymity matrix (every
 * quasi-identifier pair's *joint* k-anonymity — the dataset-level verdict).
 * A quasi-identifier is an integer column (generalized into a numeric
 * bucket) or a categorical/string column (scored on its exact values).
 * Computed in App.jsx as
 * soon as sample/profile are ready — same as useColumnRisk, and for the
 * same reason: computing this only once DiversityCheckSection mounts (i.e.,
 * only once the user opens that tab) means the per-column k-anonymity bars
 * (already resolved by then, since useColumnRisk started as soon as the
 * file loaded) visibly finish first, with the matrix lagging a beat behind
 * it instead of everything being ready together by the time the user gets
 * there.
 *
 * `removedRowIndices` (useProfiledFile.js's "remove rows from output")
 * is excluded from every row/col array before scoring —
 * same reasoning as useColumnRisk.js: a removed row is gone from the real
 * anonymize/download output, so this dataset-level analysis has to reflect
 * what's actually left, not the original upload.
 *
 * `options.quasiNames` fixes which columns are the quasi-identifiers (the Compare view measures an
 * original and its anonymized output on the same ones); without it, every column of a suitable
 * type is one. `options.bucketWidth` replaces the default range width for whole numbers. */
export function useDiversityMatrix(sample, profile, removedRowIndices, options = {}) {
  const { quasiNames, bucketWidth = RISK_BUCKET_WIDTH } = options;
  const quasiColumns =
    profile?.columns.filter(
      (c) => isQuasiIdentifierKind(c) && (!quasiNames || quasiNames.includes(c.name)),
    ) ?? [];
  const quasiKey = quasiColumns.map((c) => c.name).join("\u001f");
  const allColumns = profile?.columns ?? [];

  const [raw, setRaw] = useState(null); // { lDiversity, tCloseness, betaLikeness, deltaDisclosure, jointK, rowRisk } | null
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  // Which technique computes RowRiskSection's per-row score ("suda" | "ebm")
  // — a dependency of the effect below like sample/profile
  // are, so picking a different technique re-requests the whole matrix the
  // same way a new upload would, rather than needing a second, bespoke
  // recompute path just for this one piece of it.
  const [rowRiskModel, setRowRiskModel] = useState("suda");
  const generation = useRef(0);

  useEffect(() => {
    if (!sample || !profile || quasiColumns.length === 0) {
      setRaw(null);
      setLoading(false);
      setError(null);
      return;
    }
    const myGeneration = ++generation.current;
    setLoading(true);
    setError(null);
    const colIndex = {};
    sample.headers.forEach((name, i) => (colIndex[name] = i));
    const includedRows = removedRowIndices?.size
      ? Array.from({ length: sample.rowCount }, (_, i) => i).filter(
          (i) => !removedRowIndices.has(i),
        )
      : Array.from({ length: sample.rowCount }, (_, i) => i);
    // rows (quasi-identifier): an integer column's wasm call takes a typed
    // array; a categorical/string column stays stringified cells, same as
    // cols (candidate sensitive attribute, which can be any column type).
    // transfer collects only the includedRows-filtered copies built fresh
    // below -- never the unfiltered passthrough, which is sample's own
    // cached array shared with useColumnRisk and every other reader.
    const transfer = [];
    const rows = quasiColumns.map((c) => {
      if (c.inferred_type === "integer") {
        const values = sample.getNumericColumn(colIndex[c.name]);
        if (!removedRowIndices?.size) return { name: c.name, kind: "numeric", values };
        const owned = Float64Array.from(includedRows, (row) => values[row]);
        transfer.push(owned.buffer);
        return { name: c.name, kind: "numeric", values: owned };
      }
      return {
        name: c.name,
        kind: "string",
        values: includedRows.map((row) => sample.getCell(row, colIndex[c.name])),
      };
    });
    const cols = allColumns.map((c) => ({
      name: c.name,
      values: includedRows.map((row) => sample.getCell(row, colIndex[c.name])),
    }));
    scoreDiversityMatrix(rows, cols, bucketWidth, rowRiskModel, transfer)
      .then((result) => {
        if (generation.current !== myGeneration) return; // superseded by a newer upload
        setRaw(result);
        setLoading(false);
      })
      .catch((err) => {
        if (generation.current !== myGeneration) return;
        // Otherwise this fails silently: `raw === null` looks identical to
        // "still loading" to every consumer below (DiversityCheckSection,
        // RowRiskSection both key their skeleton/placeholder off
        // `loading || !raw`), so a real failure here used to render as a
        // permanently-spinning tab with nothing in the console to debug it.
        console.error("useDiversityMatrix: scoreDiversityMatrix failed", err);
        setRaw(null);
        setLoading(false);
        setError(String(err?.message || err));
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sample, profile, rowRiskModel, removedRowIndices, quasiKey, bucketWidth]);

  return { raw, loading, error, quasiColumns, allColumns, rowRiskModel, setRowRiskModel };
}
