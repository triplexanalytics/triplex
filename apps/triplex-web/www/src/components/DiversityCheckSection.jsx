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

/** The `cellAt` accessors and click handlers for one dataset's five matrices. A hook (not inline)
 * so the Compare view can build a second set for the other dataset. */
function useMatrixCells(raw, quasiColumns, allColumns, onJointKCellClick) {
  const lDiversityCellAt = useCallback(
    (i, j) => {
      const cell = raw?.lDiversity[i]?.[j];
      if (!cell) return null;
      return {
        risk: reciprocalRisk(cell.value),
        label: String(cell.value),
        tooltip: {
          title: `${quasiColumns[i].name} → ${allColumns[j].name}`,
          detail: `l-diversity ${cell.value}: ${lDiversityDetail(cell.value)}${
            cell.worstGroup ? " — click to filter the table to that group." : ""
          }`,
        },
        worstGroup: cell.worstGroup,
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
          detail: `t-closeness ${value.toFixed(2)}: ${tClosenessDetail(value)}${
            cell.worstGroup ? " — click to filter the table to that group." : ""
          }`,
        },
        worstGroup: cell.worstGroup,
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
          detail: `β-likeness ${value.toFixed(2)}: ${betaLikenessDetail(value)}${
            cell.worstGroup ? " — click to filter the table to that group." : ""
          }`,
        },
        worstGroup: cell.worstGroup,
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
          detail: `δ-disclosure ${value.toFixed(2)}: ${deltaDisclosureDetail(value)}${
            cell.worstGroup ? " — click to filter the table to that group." : ""
          }`,
        },
        worstGroup: cell.worstGroup,
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

  // Unlike jointK's two-column composite key, these four matrices pair one
  // quasi-identifier column (the row) against one candidate sensitive
  // column (the col) -- the worst group is a plain bucket value on the
  // row's own column alone, so filtering only ever needs one condition, no
  // U+001F split. `cellAt` is whichever of the four matrix's own accessors
  // owns this click.
  const makeSingleColumnCellClickHandler = useCallback(
    (cellAt) =>
      ({ row, col, rowName }) => {
        const cell = cellAt(row, col);
        if (!cell?.worstGroup) return;
        onJointKCellClick?.([{ columnName: rowName, bucketValue: cell.worstGroup }]);
      },
    [onJointKCellClick],
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

  return {
    lDiversityCellAt,
    tClosenessCellAt,
    betaLikenessCellAt,
    deltaDisclosureCellAt,
    jointKCellAt,
    makeSingleColumnCellClickHandler,
    handleJointKCellClick,
  };
}

/** One matrix per metric, described once so the single-dataset layout and the side-by-side
 * Compare layout render the same list. */
function matrixSpecs(cells, quasiColumns, allColumns, onJointKCellClick) {
  const rowNames = quasiColumns.map((c) => c.name);
  const colNames = allColumns.map((c) => c.name);
  const single = (cellAt) =>
    onJointKCellClick ? cells.makeSingleColumnCellClickHandler(cellAt) : undefined;
  const specs = [
    {
      key: "l",
      title: "l-diversity",
      info: L_DIVERSITY_INFO,
      rows: rowNames,
      cols: colNames,
      cellAt: cells.lDiversityCellAt,
      onCellClick: single(cells.lDiversityCellAt),
    },
    {
      key: "t",
      title: "t-closeness",
      info: T_CLOSENESS_INFO,
      rows: rowNames,
      cols: colNames,
      cellAt: cells.tClosenessCellAt,
      onCellClick: single(cells.tClosenessCellAt),
    },
    {
      key: "b",
      title: "β-likeness",
      info: BETA_LIKENESS_INFO,
      rows: rowNames,
      cols: colNames,
      cellAt: cells.betaLikenessCellAt,
      onCellClick: single(cells.betaLikenessCellAt),
    },
    {
      key: "d",
      title: "δ-disclosure",
      info: DELTA_DISCLOSURE_INFO,
      rows: rowNames,
      cols: colNames,
      cellAt: cells.deltaDisclosureCellAt,
      onCellClick: single(cells.deltaDisclosureCellAt),
    },
  ];
  if (quasiColumns.length >= 2) {
    specs.push({
      key: "k",
      title: "combined k-anonymity",
      info: JOINT_K_INFO,
      rows: rowNames,
      cols: rowNames,
      cellAt: cells.jointKCellAt,
      onCellClick: onJointKCellClick ? cells.handleJointKCellClick : undefined,
    });
  }
  return specs;
}

function MatrixOrSkeleton({ spec, ready, hoverNames, onHoverNamesChange }) {
  if (!ready) {
    return (
      <RiskMatrixSkeleton
        title={spec.title}
        rowCount={spec.rows.length}
        colCount={spec.cols.length}
      />
    );
  }
  return (
    <RiskMatrix
      title={spec.title}
      rows={spec.rows}
      cols={spec.cols}
      infoText={spec.info}
      cellAt={spec.cellAt}
      legendLowLabel="safer"
      legendHighLabel="riskier"
      highlightRowName={hoverNames?.rowName}
      highlightColName={hoverNames?.colName}
      onHoverChange={onHoverNamesChange}
      onCellClick={spec.onCellClick}
    />
  );
}

const unionNames = (a, b) => [...a, ...b.filter((n) => !a.includes(n))];

/** Redraw `spec` on `rows` x `cols` (a superset of its own axes): a pair the dataset has no score
 * for is an empty cell. Lets two datasets with different column lists share one layout. */
function alignSpec(spec, rows, cols) {
  const rowAt = rows.map((n) => spec.rows.indexOf(n));
  const colAt = cols.map((n) => spec.cols.indexOf(n));
  return {
    ...spec,
    rows,
    cols,
    cellAt: (i, j) => (rowAt[i] < 0 || colAt[j] < 0 ? null : spec.cellAt(rowAt[i], colAt[j])),
    onCellClick: undefined,
  };
}

/** The five matrices of two datasets, one metric per row with the two datasets side by side, so
 * "l-diversity" of each lines up even when their column lists differ. `left`/`right` are
 * `{ raw, loading, error, quasiColumns, allColumns }` (the diversity-matrix hook's result). */
export function DiversityCompare({ left, right, hoverNames, onHoverNamesChange }) {
  const leftCells = useMatrixCells(left.raw, left.quasiColumns, left.allColumns);
  const rightCells = useMatrixCells(right.raw, right.quasiColumns, right.allColumns);
  const leftSpecs = matrixSpecs(leftCells, left.quasiColumns, left.allColumns);
  const rightSpecs = matrixSpecs(rightCells, right.quasiColumns, right.allColumns);
  const keys = [...new Set([...leftSpecs, ...rightSpecs].map((s) => s.key))];
  const ready = (side) => !side.loading && !!side.raw && !side.error;
  return (
    <div className="compare-matrix-rows">
      {keys.map((key) => {
        let l = leftSpecs.find((s) => s.key === key);
        let r = rightSpecs.find((s) => s.key === key);
        if (l && r) {
          // Same axes on both sides (columns one dataset lacks show as empty cells), so the two
          // matrices have the same size and every column sits in the same place.
          const rows = unionNames(l.rows, r.rows);
          const cols = unionNames(l.cols, r.cols);
          l = alignSpec(l, rows, cols);
          r = alignSpec(r, rows, cols);
        }
        return (
          <div className="compare-pair" key={key}>
            <div className="compare-side">
              {l ? (
                <MatrixOrSkeleton
                  spec={l}
                  ready={ready(left)}
                  hoverNames={hoverNames}
                  onHoverNamesChange={onHoverNamesChange}
                />
              ) : (
                <p className="synth-hint">Not available for this dataset.</p>
              )}
            </div>
            <div className="compare-side">
              {r ? (
                <MatrixOrSkeleton
                  spec={r}
                  ready={ready(right)}
                  hoverNames={hoverNames}
                  onHoverNamesChange={onHoverNamesChange}
                />
              ) : (
                <p className="synth-hint">Not available for this dataset.</p>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

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
  const cells = useMatrixCells(raw, quasiColumns, allColumns, onJointKCellClick);
  if (quasiColumns.length === 0) return null;

  if (error) {
    return (
      <div className="diversity-check-section">
        <h3>Diversity check</h3>
        <p className="banner banner-error">
          Couldn't score this file's diversity/row risk: {error}
        </p>
      </div>
    );
  }

  const specs = matrixSpecs(cells, quasiColumns, allColumns, onJointKCellClick);
  const showJointK = quasiColumns.length >= 2;

  return (
    <div className="diversity-check-section">
      <h3>Diversity check</h3>
      <p className="diversity-check-intro">
        k-anonymity (above) only says how big each generalized group is. These say whether everyone
        in that group also shares the same sensitive value: one row per quasi-identifier column, one
        column per candidate sensitive column.
        {showJointK &&
          " Combined k-anonymity (right) scores every quasi-identifier column against every other one, together instead of alone — the dataset-level verdict."}{" "}
        Hover a cell for the pair it scores.
      </p>
      <div className="risk-matrix-pair">
        {specs.map((spec) => (
          <MatrixOrSkeleton
            key={spec.key}
            spec={spec}
            ready={!loading && !!raw}
            hoverNames={hoverNames}
            onHoverNamesChange={onHoverNamesChange}
          />
        ))}
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
