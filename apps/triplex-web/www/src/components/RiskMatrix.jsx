import { useEffect, useRef, useState } from "react";
import { truncateToWidth } from "../utils";
import { oceanDriftAlpha, readRiskColorStops } from "../state/palette";
import {
  CELL_SIZE,
  CELL_GAP,
  ROW_LABEL_WIDTH,
  COL_LABEL_HEIGHT,
  computeMatrixSize,
} from "../state/matrixLayout";
import { NO_DATA_COLOR } from "./RiskMatrix.const";
import InfoTrigger from "./InfoTrigger";

/** Canvas-rendered risk heatmap: `rows` (quasi-identifier columns) x `cols`
 * (candidate sensitive columns), one cell per pair. `cellAt(rowIndex,
 * colIndex)` returns `{ risk, tooltip, label }` or `null` for a pair with
 * nothing to show (e.g. the same column picked on both axes). `risk` is a
 * plain number in [0, 1] — 0 safest, 1 riskiest — already normalized by the
 * caller from whatever the underlying metric's own range is, since that
 * normalization differs per metric (l-diversity vs. t-closeness). `label`
 * is the short display form of the metric's actual value (e.g. `"3"`,
 * `"0.42""), shown on the cell itself on hover — the color ramp alone
 * can't distinguish two risks that round to a similar shade.
 *
 * Plain 2D canvas, not a charting library: a static grid of a few hundred
 * flat-colored rects with a hover tooltip has no need for a WebGL scene
 * graph.
 *
 * The hovered cell's value is drawn on a second, transparent canvas
 * layered on top rather than baked into the main draw pass: fading it in
 * via a CSS `opacity` transition on that whole layer is simple and smooth,
 * where animating an opacity ramp for one rect inside a single canvas
 * would need its own requestAnimationFrame loop for no real benefit here.
 * The main canvas's (expensive, whole-grid) redraw is untouched by
 * hovering — only the small overlay layer repaints per mouse move.
 *
 * Rows and cols can be the same list (e.g. the combined-k-anonymity matrix
 * pairs every quasi-identifier column against every other one) — `cellAt`
 * deciding what a same-name pair means (typically `null`, nothing to show)
 * is entirely the caller's concern, this component just draws whatever
 * grid it's given.
 *
 * `highlightRowName`/`highlightColName` (row/col label strings, not
 * indices — different matrices don't share one index space, but several
 * share row and column names) let a hover in a sibling matrix light up the
 * matching row/column here too; `onHoverChange` reports this matrix's own
 * hover up so a parent rendering several matrices side by side can wire
 * them together (see DiversityCheckSection). All optional — a lone matrix
 * just doesn't cross-highlight anything.
 *
 * `onCellClick`, if given, fires `{ row, col, rowName, colName }` (indices
 * *and* names — the caller already has its own `cellAt` closure to look up
 * whatever cell detail it needs by index, so this doesn't also thread the
 * cell's data back out). Also switches the cursor to a pointer over cells
 * (not labels) as a click affordance. */
export default function RiskMatrix({
  title,
  rows,
  cols,
  cellAt,
  legendLowLabel,
  legendHighLabel,
  infoText,
  highlightRowName,
  highlightColName,
  onHoverChange,
  onCellClick,
}) {
  const canvasRef = useRef(null);
  const overlayRef = useRef(null);
  const [hover, setHover] = useState(null); // { x, y, row, col, label, tooltip } | null

  const { width, height, legendWidth } = computeMatrixSize(rows.length, cols.length);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    const ctx = canvas.getContext("2d");
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, width, height);
    ctx.font = "11px system-ui, -apple-system, sans-serif";
    ctx.textBaseline = "middle";

    const { riskColor, hexStops } = readRiskColorStops("matrix");
    const textColor =
      getComputedStyle(document.documentElement).getPropertyValue("--text-muted").trim() ||
      "#5b6270";
    const borderColor =
      getComputedStyle(document.documentElement).getPropertyValue("--border").trim() || "#e1e4e9";

    // Column labels, rotated -45deg, anchored just above the grid.
    cols.forEach((col, j) => {
      const cx = ROW_LABEL_WIDTH + j * CELL_SIZE + CELL_SIZE / 2;
      ctx.save();
      ctx.translate(cx, COL_LABEL_HEIGHT - 6);
      ctx.rotate(-Math.PI / 4);
      ctx.textAlign = "left";
      ctx.fillStyle = textColor;
      const { text } = truncateToWidth(ctx, col, COL_LABEL_HEIGHT - 10);
      ctx.fillText(text, 0, 0);
      ctx.restore();
    });

    // Row labels, right-aligned against the grid's left edge.
    rows.forEach((row, i) => {
      const cy = COL_LABEL_HEIGHT + i * CELL_SIZE + CELL_SIZE / 2;
      ctx.textAlign = "right";
      ctx.fillStyle = textColor;
      const { text } = truncateToWidth(ctx, row, ROW_LABEL_WIDTH - 10);
      ctx.fillText(text, ROW_LABEL_WIDTH - 8, cy);
    });

    // Cells. Slightly rounded with a light border — the ramp's safe end is
    // now pure white, so every cell needs its own edge or it disappears
    // into the card's (also white) surface.
    const CELL_RADIUS = 3;
    rows.forEach((_, i) => {
      cols.forEach((_, j) => {
        const cell = cellAt(i, j);
        const x = ROW_LABEL_WIDTH + j * CELL_SIZE;
        const y = COL_LABEL_HEIGHT + i * CELL_SIZE;
        ctx.beginPath();
        ctx.roundRect(x, y, CELL_SIZE - CELL_GAP, CELL_SIZE - CELL_GAP, CELL_RADIUS);
        ctx.fillStyle = cell ? riskColor(cell.risk) : NO_DATA_COLOR;
        ctx.fill();
        ctx.strokeStyle = borderColor;
        ctx.lineWidth = 1;
        ctx.stroke();
      });
    });

    // Legend: a smooth gradient across the same three stops, min/max labels.
    const legendX = ROW_LABEL_WIDTH;
    const legendY = COL_LABEL_HEIGHT + rows.length * CELL_SIZE + 14;
    const gradient = ctx.createLinearGradient(legendX, 0, legendX + legendWidth, 0);
    gradient.addColorStop(0, hexStops[0]);
    gradient.addColorStop(1, hexStops[1]);
    ctx.fillStyle = gradient;
    ctx.fillRect(legendX, legendY, legendWidth, 10);
    ctx.fillStyle = textColor;
    ctx.textAlign = "left";
    ctx.fillText(legendLowLabel, legendX, legendY + 22);
    ctx.textAlign = "right";
    ctx.fillText(legendHighLabel, legendX + legendWidth, legendY + 22);
  }, [rows, cols, cellAt, width, height, legendWidth, legendLowLabel, legendHighLabel]);

  // Cross-component hover reports (a sibling matrix, or now the table
  // header) get dropped if called synchronously from this canvas's native-
  // event-routed mousemove/mouseleave handler -- confirmed by direct
  // testing: the exact same setter, called with the exact same value, from
  // a DOM div's onMouseEnter (ColumnCard) or called directly, commits
  // normally; called from here, it doesn't (no re-render, no effect, no
  // thrown error) unless deferred past the current synchronous event
  // stack. Root cause not pinned down further than that; the local `hover`
  // tooltip state (setHover, below) isn't affected, only this callback.
  const reportHover = (value) => setTimeout(() => onHoverChange?.(value), 0);

  const onMouseMove = (e) => {
    const rect = canvasRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const col = Math.floor((x - ROW_LABEL_WIDTH) / CELL_SIZE);
    const row = Math.floor((y - COL_LABEL_HEIGHT) / CELL_SIZE);
    const overCell = row >= 0 && row < rows.length && col >= 0 && col < cols.length;
    // Hovering a label alone (not a cell) highlights just that row/col —
    // x < ROW_LABEL_WIDTH puts col negative, y < COL_LABEL_HEIGHT puts row
    // negative, so these never overlap with `overCell` or each other.
    const overRowLabel =
      !overCell && x >= 0 && x < ROW_LABEL_WIDTH && row >= 0 && row < rows.length;
    const overColLabel =
      !overCell && y >= 0 && y < COL_LABEL_HEIGHT && col >= 0 && col < cols.length;

    if (overCell) {
      // Reported even for a cell with nothing to show (`cellAt` -> null):
      // the row/col a sibling matrix should highlight doesn't depend on
      // whether this particular pair has data.
      reportHover({ rowName: rows[row], colName: cols[col] });
      const cell = cellAt(row, col);
      if (!cell) {
        setHover(null);
        return;
      }
      setHover({ x: e.clientX, y: e.clientY, row, col, label: cell.label, tooltip: cell.tooltip });
    } else if (overRowLabel) {
      reportHover({ rowName: rows[row], colName: null });
      setHover(null);
    } else if (overColLabel) {
      reportHover({ rowName: null, colName: cols[col] });
      setHover(null);
    } else {
      setHover(null);
      reportHover(null);
    }
  };

  const onMouseLeave = () => {
    setHover(null);
    reportHover(null);
  };

  const onClick = (e) => {
    if (!onCellClick) return;
    const rect = canvasRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const col = Math.floor((x - ROW_LABEL_WIDTH) / CELL_SIZE);
    const row = Math.floor((y - COL_LABEL_HEIGHT) / CELL_SIZE);
    const overCell = row >= 0 && row < rows.length && col >= 0 && col < cols.length;
    if (overCell) {
      onCellClick({ row, col, rowName: rows[row], colName: cols[col] });
    }
  };

  // Redraws only the small overlay layer, not the whole (expensive) grid —
  // the value fades in/out via the layer's own CSS opacity transition
  // (`.risk-matrix-hover-value`'s `opacity` class toggle below), not a
  // per-frame animation here.
  useEffect(() => {
    const overlay = overlayRef.current;
    if (!overlay) return;
    const dpr = window.devicePixelRatio || 1;
    overlay.width = width * dpr;
    overlay.height = height * dpr;
    overlay.style.width = `${width}px`;
    overlay.style.height = `${height}px`;
    const ctx = overlay.getContext("2d");
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, width, height);

    const rootStyle = getComputedStyle(document.documentElement);
    const textColor = rootStyle.getPropertyValue("--text").trim() || "#1a1d24";

    // Cross-matrix highlight band, by row/col *name* (not index — a
    // sibling matrix's rows/cols aren't necessarily this matrix's own),
    // drawn on this cheap overlay layer rather than the main grid so a
    // hover over a sibling matrix doesn't trigger every matrix's
    // expensive full redraw.
    const highlightRow = highlightRowName != null ? rows.indexOf(highlightRowName) : -1;
    const highlightCol = highlightColName != null ? cols.indexOf(highlightColName) : -1;
    if (highlightRow >= 0 || highlightCol >= 0) {
      const gridWidth = ROW_LABEL_WIDTH + cols.length * CELL_SIZE;
      const gridBottom = COL_LABEL_HEIGHT + rows.length * CELL_SIZE;
      ctx.fillStyle = oceanDriftAlpha(0.09);
      if (highlightRow >= 0) {
        ctx.fillRect(0, COL_LABEL_HEIGHT + highlightRow * CELL_SIZE, gridWidth, CELL_SIZE);
      }
      if (highlightCol >= 0) {
        ctx.fillRect(ROW_LABEL_WIDTH + highlightCol * CELL_SIZE, 0, CELL_SIZE, gridBottom);
      }

      // Bold the highlighted row/col's own label, redrawn here on the
      // cheap overlay layer (on top of the regular-weight one already
      // baked into the main canvas below, at the exact same position) —
      // rebolding it on the main canvas would mean redrawing the whole
      // (expensive) grid on every hover, the exact cost this overlay
      // layer exists to avoid.
      ctx.font = "700 11px system-ui, -apple-system, sans-serif";
      ctx.fillStyle = textColor;
      if (highlightRow >= 0) {
        const cy = COL_LABEL_HEIGHT + highlightRow * CELL_SIZE + CELL_SIZE / 2;
        ctx.textAlign = "right";
        ctx.textBaseline = "middle";
        const { text } = truncateToWidth(ctx, rows[highlightRow], ROW_LABEL_WIDTH - 10);
        ctx.fillText(text, ROW_LABEL_WIDTH - 8, cy);
      }
      if (highlightCol >= 0) {
        const cx = ROW_LABEL_WIDTH + highlightCol * CELL_SIZE + CELL_SIZE / 2;
        ctx.save();
        ctx.translate(cx, COL_LABEL_HEIGHT - 6);
        ctx.rotate(-Math.PI / 4);
        ctx.textAlign = "left";
        ctx.textBaseline = "middle";
        const { text } = truncateToWidth(ctx, cols[highlightCol], COL_LABEL_HEIGHT - 10);
        ctx.fillText(text, 0, 0);
        ctx.restore();
      }
    }

    if (!hover || hover.label === undefined) return;
    const cx = ROW_LABEL_WIDTH + hover.col * CELL_SIZE + (CELL_SIZE - CELL_GAP) / 2;
    const cy = COL_LABEL_HEIGHT + hover.row * CELL_SIZE + (CELL_SIZE - CELL_GAP) / 2;
    const outlineColor = rootStyle.getPropertyValue("--surface").trim() || "#ffffff";
    ctx.font = "600 11px system-ui, -apple-system, sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    // A cell's fill can be anywhere from near-white to deep red, so plain
    // themed text would vanish against half the ramp — an outline in the
    // theme's surface color (the opposite end from its own text color in
    // both light and dark mode) keeps the value legible over any cell color.
    ctx.lineWidth = 3;
    ctx.strokeStyle = outlineColor;
    ctx.strokeText(hover.label, cx, cy);
    ctx.fillStyle = textColor;
    ctx.fillText(hover.label, cx, cy);
  }, [hover, width, height, rows, cols, highlightRowName, highlightColName]);

  // A centered tooltip (translate(-50%, ...)) overflows the viewport for a
  // cell near the left or right edge — clamp the anchor inward by a rough
  // half-width estimate rather than letting it run off-screen.
  const TOOLTIP_HALF_WIDTH = 130; // half of .bin-tooltip.wrap's 260px max-width
  const clampedX = hover
    ? Math.max(
        TOOLTIP_HALF_WIDTH + 8,
        Math.min(hover.x, window.innerWidth - TOOLTIP_HALF_WIDTH - 8),
      )
    : 0;

  return (
    <div className="risk-matrix">
      <h4>
        {title}
        {infoText && (
          <>
            {" "}
            <InfoTrigger tooltip={infoText} align="left" />
          </>
        )}
      </h4>
      <div className="risk-matrix-canvas-stack">
        <canvas
          ref={canvasRef}
          className={onCellClick ? "risk-matrix-clickable" : undefined}
          onMouseMove={onMouseMove}
          onMouseLeave={onMouseLeave}
          onClick={onClick}
        />
        <canvas
          ref={overlayRef}
          className={`risk-matrix-hover-value${
            hover || highlightRowName != null || highlightColName != null ? " visible" : ""
          }`}
        />
      </div>
      {hover && (
        <div
          className="bin-tooltip wrap"
          style={{
            position: "fixed",
            left: clampedX,
            top: hover.y,
            transform: "translate(-50%, -100%)",
          }}
        >
          <strong>{hover.tooltip.title}</strong>
          <span>{hover.tooltip.detail}</span>
        </div>
      )}
    </div>
  );
}
