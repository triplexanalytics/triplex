// Layout constants shared between RiskMatrix's actual canvas and its
// loading skeleton, so the skeleton placeholder is exactly the size the
// real matrix will be instead of a guess that jumps once data arrives.
export const CELL_SIZE = 26;
export const CELL_GAP = 2;
export const ROW_LABEL_WIDTH = 130;
export const COL_LABEL_HEIGHT = 90;
export const LEGEND_HEIGHT = 34;
export const PADDING = 8;

export function computeMatrixSize(rowCount, colCount) {
  const legendWidth = Math.max(160, colCount * CELL_SIZE);
  return {
    width: ROW_LABEL_WIDTH + legendWidth + PADDING,
    height: COL_LABEL_HEIGHT + rowCount * CELL_SIZE + LEGEND_HEIGHT + PADDING,
    legendWidth,
  };
}
