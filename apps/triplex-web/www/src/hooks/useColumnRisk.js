import { useEffect, useRef, useState } from "react";
import { scoreColumnRisk } from "../state/datasetCardClient";

// Pre-anonymize privacy-risk advisory: a fixed
// generalization width, since neither the Table tab's header nor the
// Column view has a per-column bucket-width control the way the actual
// anonymize recipe does — this is an advisory, not the real masking step.
const RISK_BUCKET_WIDTH = 10;

/** Per-column k-anonymity, shared by the Table tab's header bar and the
 * Column view's per-card bar so both read the same computation instead of
 * each running its own worker request for the same data. Integer columns
 * only: `bucket_value` only generalizes whole numbers. Runs in a worker
 * (datasetCardClient) so a large file's worth of scoring never blocks the
 * main thread; `generation` drops a result that arrives after a newer
 * upload has already superseded it. */
export function useColumnRisk(sample, profile) {
  const [riskByName, setRiskByName] = useState({});
  const [riskLoading, setRiskLoading] = useState(false);
  const generation = useRef(0);

  useEffect(() => {
    if (!sample || !profile) {
      setRiskByName({});
      setRiskLoading(false);
      return;
    }
    const integerCols = profile.columns.filter((c) => c.inferred_type === "integer");
    if (integerCols.length === 0) {
      setRiskByName({});
      setRiskLoading(false);
      return;
    }
    const myGeneration = ++generation.current;
    setRiskLoading(true);
    const colIndex = {};
    sample.headers.forEach((name, i) => (colIndex[name] = i));
    const columns = integerCols.map((c) => ({
      name: c.name,
      values: sample.getNumericColumn(colIndex[c.name]),
    }));
    scoreColumnRisk(columns, RISK_BUCKET_WIDTH)
      .then((result) => {
        if (generation.current !== myGeneration) return; // superseded by a newer upload
        setRiskByName(result);
        setRiskLoading(false);
      })
      .catch(() => {
        if (generation.current !== myGeneration) return;
        setRiskByName({});
        setRiskLoading(false);
      });
  }, [sample, profile]);

  return { riskByName, riskLoading };
}
