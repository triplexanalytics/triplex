import { CELL_SIZE, CELL_GAP, ROW_LABEL_WIDTH, COL_LABEL_HEIGHT, computeMatrixSize } from "../state/matrixLayout";

/** Placeholder shown in place of a `RiskMatrix` while its scores are still
 * being computed in the worker — sized via the same `computeMatrixSize`
 * the real matrix uses, so the layout doesn't jump once data arrives.
 * Renders `rowCount x colCount` pulsing placeholder cells at the exact
 * positions the real heatmap's cells will occupy, so the shape reads as
 * "a matrix is coming" rather than an undifferentiated gray block — the
 * spinner + label on top still carries the "still loading" half of that,
 * same as before. */
export default function RiskMatrixSkeleton({ title, rowCount, colCount }) {
  const { width, height } = computeMatrixSize(rowCount, colCount);
  const cells = [];
  for (let r = 0; r < rowCount; r++) {
    for (let c = 0; c < colCount; c++) {
      cells.push(
        <div
          key={`${r}-${c}`}
          className="risk-matrix-skeleton-cell skeleton-block"
          style={{
            left: ROW_LABEL_WIDTH + c * CELL_SIZE,
            top: COL_LABEL_HEIGHT + r * CELL_SIZE,
            width: CELL_SIZE - CELL_GAP,
            height: CELL_SIZE - CELL_GAP,
            // Staggers the pulse across the grid instead of every cell
            // brightening/dimming in lockstep — reads as more "alive"
            // without being distracting.
            animationDelay: `${((r * colCount + c) % 7) * 0.08}s`,
          }}
        />,
      );
    }
  }
  return (
    <div className="risk-matrix">
      <h4>{title}</h4>
      <div className="risk-matrix-skeleton" style={{ width, height }}>
        {cells}
        <div className="skeleton-block-loading">
          <div className="spinner" aria-hidden="true" />
          <p>Computing…</p>
        </div>
      </div>
    </div>
  );
}
