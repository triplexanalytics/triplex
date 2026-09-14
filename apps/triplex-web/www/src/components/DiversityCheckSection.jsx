import { useCallback } from "react";
import RiskMatrix from "./RiskMatrix";
import RiskMatrixSkeleton from "./RiskMatrixSkeleton";
import RowRiskSection from "./RowRiskSection";
import { reciprocalRisk } from "../state/palette";
import {
  L_DIVERSITY_INFO,
  T_CLOSENESS_INFO,
  BETA_LIKENESS_INFO,
  DELTA_DISCLOSURE_INFO,
  JOINT_K_INFO,
  lDiversityDetail,
  tClosenessDetail,
  betaLikenessDetail,
  deltaDisclosureDetail,
  jointKDetail,
} from "../state/riskExplanations";

// beta-likeness/delta-disclosure are "lower is safer" like t-closeness (not
// "higher is safer" like k-anonymity/l-diversity, which use `reciprocalRisk`
// to invert), but unbounded above (a log-ratio, not a 0-1 distance) — scale
// against a reasonable ceiling instead of t-closeness's implicit 0-1 range,
// so one extreme outlier doesn't wash out the color contrast between every
// other, more typical cell. ln(20) ~= 3 is already an enormous belief shift
// (20x more/less likely than the dataset baseline), generous enough not to
// clip realistic data.
const BELIEF_SHIFT_CEILING = Math.log(20);

/** Dataset Anonymity Analysis tab: up to five heatmaps below the
 * per-column k-anonymity cards, plus a row-level risk list. l-diversity,
 * t-closeness, β-likeness, and δ-disclosure (the design notes adding the
 * latter two) each cross every quasi-identifier column (integer,
 * generalized into a bucket, or categorical/string, scored on its exact
 * values) against every candidate sensitive column — t-closeness reports
 * one aggregate "how different is this group's mix" distance,
 * β-likeness/δ-disclosure report the sharper "single biggest belief shift
 * about one value" instead. Combined k-anonymity (the design notes
 * "dataset-level aggregation" gap) crosses every quasi-identifier column
 * against every *other* quasi-identifier column, scoring the pair
 * together instead of alone — the dataset-level verdict a single column's
 * score can't give, since two columns that each look safe alone can
 * single out almost everyone combined. A single chooser can't show any of
 * this: each needs a *pair*, and a matrix is the only view that shows
 * every pair at once instead of one at a time. `RowRiskSection` below is
 * the odd one out on purpose: a per-*row* SUDA score, not a matrix at all
 * — see its own doc comment.
 *
 * Purely presentational — `useDiversityMatrix` in App.jsx owns the actual
 * computation, started as soon as the file loads rather than when this
 * component mounts, so it's ready (or close to it) by the time the user
 * opens this tab instead of visibly lagging behind the k-anonymity bars.
 *
 * `hoverNames`/`onHoverNamesChange` are lifted one level further than
 * before, into App.jsx: these five matrices used to be the only place
 * sharing row/column names across siblings, but a `ColumnCard` hover
 * (`ColumnViewTab`, App.jsx's other child in this tab) now needs to reach
 * them too, so their common parent owns the state instead of this
 * component. */
export default function DiversityCheckSection({
  raw,
  loading,
  error,
  quasiColumns,
  allColumns,
  hoverNames,
  onHoverNamesChange,
  sample,
  rowRiskModel,
  onRowRiskModelChange,
  onJointKCellClick,
}) {
  const lDiversityCellAt = useCallback(
    (i, j) => {
      const cell = raw?.lDiversity[i]?.[j];
      if (!cell) return null;
      return {
        risk: reciprocalRisk(cell.value),
        label: String(cell.value),
        tooltip: {
          title: `${quasiColumns[i].name} → ${allColumns[j].name}`,
          detail: `l-diversity ${cell.value}: ${lDiversityDetail(cell.value)}`,
        },
      };
    },
    [raw, quasiColumns, allColumns],
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
          title: `${quasiColumns[i].name} → ${allColumns[j].name}`,
          detail: `t-closeness ${value.toFixed(2)}: ${tClosenessDetail(value)}`,
        },
      };
    },
    [raw, quasiColumns, allColumns],
  );

  const betaLikenessCellAt = useCallback(
    (i, j) => {
      const cell = raw?.betaLikeness[i]?.[j];
      if (!cell) return null;
      const value = Math.max(0, cell.value);
      return {
        risk: Math.min(1, value / BELIEF_SHIFT_CEILING),
        label: value.toFixed(2),
        tooltip: {
          title: `${quasiColumns[i].name} → ${allColumns[j].name}`,
          detail: `β-likeness ${value.toFixed(2)}: ${betaLikenessDetail(value)}`,
        },
      };
    },
    [raw, quasiColumns, allColumns],
  );

  const deltaDisclosureCellAt = useCallback(
    (i, j) => {
      const cell = raw?.deltaDisclosure[i]?.[j];
      if (!cell) return null;
      const value = Math.max(0, cell.value);
      return {
        risk: Math.min(1, value / BELIEF_SHIFT_CEILING),
        label: value.toFixed(2),
        tooltip: {
          title: `${quasiColumns[i].name} → ${allColumns[j].name}`,
          detail: `δ-disclosure ${value.toFixed(2)}: ${deltaDisclosureDetail(value)}`,
        },
      };
    },
    [raw, quasiColumns, allColumns],
  );

  const jointKCellAt = useCallback(
    (i, j) => {
      const cell = raw?.jointK[i]?.[j];
      if (!cell) return null;
      return {
        risk: reciprocalRisk(cell.value),
        label: String(cell.value),
        tooltip: {
          title: `${quasiColumns[i].name} + ${quasiColumns[j].name}`,
          detail: `combined k-anonymity ${cell.value}: ${jointKDetail(cell.value)}${
            cell.worstGroup ? " — click to filter the table to that group." : ""
          }`,
        },
        worstGroup: cell.worstGroup,
      };
    },
    [raw, quasiColumns],
  );

  // jointKMatrix (the worker) only computes each pair once (i < j) and
  // mirrors it to (j, i) — so a clicked cell's own worstGroup was composed
  // as [bucketed[i], bucketed[j]] with i the *smaller* index, not
  // necessarily this cell's row. Reorder the split back to [row's own
  // bucket, col's own bucket] accordingly before handing it up.
  const handleJointKCellClick = useCallback(
    ({ row, col, rowName, colName }) => {
      const cell = jointKCellAt(row, col);
      if (!cell?.worstGroup) return;
      const [first, second] = cell.worstGroup.split("");
      const [rowBucket, colBucket] = row < col ? [first, second] : [second, first];
      onJointKCellClick?.([
        { columnName: rowName, bucketValue: rowBucket },
        { columnName: colName, bucketValue: colBucket },
      ]);
    },
    [jointKCellAt, onJointKCellClick],
  );

  if (quasiColumns.length === 0) return null;

  if (error) {
    return (
      <div className="diversity-check-section">
        <h3>Diversity check</h3>
        <p className="banner banner-error">Couldn't score this file's diversity/row risk: {error}</p>
      </div>
    );
  }

  const rowNames = quasiColumns.map((c) => c.name);
  const colNames = allColumns.map((c) => c.name);
  const showJointK = quasiColumns.length >= 2;

  return (
    <div className="diversity-check-section">
      <h3>Diversity check</h3>
      <p className="diversity-check-intro">
        k-anonymity (above) only says how big each generalized group is. These say whether
        everyone in that group also shares the same sensitive value: one row per
        quasi-identifier column, one column per candidate sensitive column.
        {showJointK &&
          " Combined k-anonymity (right) scores every quasi-identifier column against every other one, together instead of alone — the dataset-level verdict."}
        {" "}Hover a cell for the pair it scores.
      </p>
      <div className="risk-matrix-pair">
        {loading || !raw ? (
          <>
            <RiskMatrixSkeleton title="l-diversity" rowCount={rowNames.length} colCount={colNames.length} />
            <RiskMatrixSkeleton title="t-closeness" rowCount={rowNames.length} colCount={colNames.length} />
            <RiskMatrixSkeleton title="β-likeness" rowCount={rowNames.length} colCount={colNames.length} />
            <RiskMatrixSkeleton title="δ-disclosure" rowCount={rowNames.length} colCount={colNames.length} />
            {showJointK && (
              <RiskMatrixSkeleton title="combined k-anonymity" rowCount={rowNames.length} colCount={rowNames.length} />
            )}
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
              highlightRowName={hoverNames?.rowName}
              highlightColName={hoverNames?.colName}
              onHoverChange={onHoverNamesChange}
            />
            <RiskMatrix
              title="t-closeness"
              rows={rowNames}
              cols={colNames}
              infoText={T_CLOSENESS_INFO}
              cellAt={tClosenessCellAt}
              legendLowLabel="safer"
              legendHighLabel="riskier"
              highlightRowName={hoverNames?.rowName}
              highlightColName={hoverNames?.colName}
              onHoverChange={onHoverNamesChange}
            />
            <RiskMatrix
              title="β-likeness"
              rows={rowNames}
              cols={colNames}
              infoText={BETA_LIKENESS_INFO}
              cellAt={betaLikenessCellAt}
              legendLowLabel="safer"
              legendHighLabel="riskier"
              highlightRowName={hoverNames?.rowName}
              highlightColName={hoverNames?.colName}
              onHoverChange={onHoverNamesChange}
            />
            <RiskMatrix
              title="δ-disclosure"
              rows={rowNames}
              cols={colNames}
              infoText={DELTA_DISCLOSURE_INFO}
              cellAt={deltaDisclosureCellAt}
              legendLowLabel="safer"
              legendHighLabel="riskier"
              highlightRowName={hoverNames?.rowName}
              highlightColName={hoverNames?.colName}
              onHoverChange={onHoverNamesChange}
            />
            {showJointK && (
              <RiskMatrix
                title="combined k-anonymity"
                rows={rowNames}
                cols={rowNames}
                infoText={JOINT_K_INFO}
                cellAt={jointKCellAt}
                legendLowLabel="safer"
                legendHighLabel="riskier"
                highlightRowName={hoverNames?.rowName}
                highlightColName={hoverNames?.colName}
                onHoverChange={onHoverNamesChange}
                onCellClick={onJointKCellClick ? handleJointKCellClick : undefined}
              />
            )}
          </>
        )}
      </div>
      <RowRiskSection
        raw={raw}
        loading={loading}
        quasiColumns={quasiColumns}
        sample={sample}
        model={rowRiskModel}
        onModelChange={onRowRiskModelChange}
      />
    </div>
  );
}
