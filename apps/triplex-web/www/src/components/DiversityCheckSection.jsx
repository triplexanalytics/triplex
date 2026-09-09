import { useCallback } from "react";
import RiskMatrix from "./RiskMatrix";
import RiskMatrixSkeleton from "./RiskMatrixSkeleton";
import { reciprocalRisk } from "../state/riskColor";
import {
  L_DIVERSITY_INFO,
  T_CLOSENESS_INFO,
  lDiversityDetail,
  tClosenessDetail,
} from "../state/riskExplanations";

/** Dataset Anonymity Analysis tab: two heatmaps below the per-column
 * k-anonymity cards — every bucketable (integer) column crossed with every
 * candidate sensitive column, scored for l-diversity and t-closeness. A
 * single chooser can't show this: unlike k-anonymity (needs only the one
 * column), these two need a *pair*, and a matrix is the only view that
 * shows every pair at once instead of one at a time.
 *
 * Purely presentational — `useDiversityMatrix` in App.jsx owns the actual
 * computation, started as soon as the file loads rather than when this
 * component mounts, so it's ready (or close to it) by the time the user
 * opens this tab instead of visibly lagging behind the k-anonymity bars. */
export default function DiversityCheckSection({ raw, loading, integerColumns, allColumns }) {
  const lDiversityCellAt = useCallback(
    (i, j) => {
      const cell = raw?.lDiversity[i]?.[j];
      if (!cell) return null;
      return {
        risk: reciprocalRisk(cell.value),
        label: String(cell.value),
        tooltip: {
          title: `${integerColumns[i].name} → ${allColumns[j].name}`,
          detail: `l-diversity ${cell.value}: ${lDiversityDetail(cell.value)}`,
        },
      };
    },
    [raw, integerColumns, allColumns],
  );

  const tClosenessCellAt = useCallback(
    (i, j) => {
      const cell = raw?.tCloseness[i]?.[j];
      if (!cell) return null;
      const value = Math.max(0, Math.min(1, cell.value));
      return {
        risk: value,
        label: value.toFixed(2),
        tooltip: {
          title: `${integerColumns[i].name} → ${allColumns[j].name}`,
          detail: `t-closeness ${value.toFixed(2)}: ${tClosenessDetail(value)}`,
        },
      };
    },
    [raw, integerColumns, allColumns],
  );

  if (integerColumns.length === 0) return null;

  const rowNames = integerColumns.map((c) => c.name);
  const colNames = allColumns.map((c) => c.name);

  return (
    <div className="diversity-check-section">
      <h3>Diversity check</h3>
      <p className="diversity-check-intro">
        k-anonymity (above) only says how big each generalized group is. These say whether
        everyone in that group also shares the same sensitive value: one row per
        quasi-identifier column, one column per candidate sensitive column. Hover a cell for the
        pair it scores.
      </p>
      <div className="risk-matrix-pair">
        {loading || !raw ? (
          <>
            <RiskMatrixSkeleton title="l-diversity" rowCount={rowNames.length} colCount={colNames.length} />
            <RiskMatrixSkeleton title="t-closeness" rowCount={rowNames.length} colCount={colNames.length} />
          </>
        ) : (
          <>
            <RiskMatrix
              title="l-diversity"
              rows={rowNames}
              cols={colNames}
              infoText={L_DIVERSITY_INFO}
              cellAt={lDiversityCellAt}
              legendLowLabel="safer"
              legendHighLabel="riskier"
            />
            <RiskMatrix
              title="t-closeness"
              rows={rowNames}
              cols={colNames}
              infoText={T_CLOSENESS_INFO}
              cellAt={tClosenessCellAt}
              legendLowLabel="safer"
              legendHighLabel="riskier"
            />
          </>
        )}
      </div>
    </div>
  );
}
