import { useEffect, useRef, useState } from "react";
import { truncateToWidth } from "../utils";
import { readRiskColorStops } from "../state/palette";
import { CELL_SIZE, CELL_GAP, ROW_LABEL_WIDTH, COL_LABEL_HEIGHT, computeMatrixSize } from "../state/matrixLayout";
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
 * graph. Re-reads its color stops (and redraws) on a light/dark theme
 * change since canvas paint doesn't follow CSS custom properties on its
 * own the way DOM elements do.
 *
 * The hovered cell's value is drawn on a second, transparent canvas
 * layered on top rather than baked into the main draw pass: fading it in
 * via a CSS `opacity` transition on that whole layer is simple and smooth,
 * where animating an opacity ramp for one rect inside a single canvas
 * would need its own requestAnimationFrame loop for no real benefit here.
 * The main canvas's (expensive, whole-grid) redraw is untouched by
 * hovering — only the small overlay layer repaints per mouse move. */
export default function RiskMatrix({ title, rows, cols, cellAt, legendLowLabel, legendHighLabel, infoText }) {
  const canvasRef = useRef(null);
  const overlayRef = useRef(null);
  const [hover, setHover] = useState(null); // { x, y, row, col, label, tooltip } | null
  const [colorTick, setColorTick] = useState(0);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setColorTick((t) => t + 1);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

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
    const textColor = getComputedStyle(document.documentElement).getPropertyValue("--text-muted").trim() || "#5b6270";
    const borderColor = getComputedStyle(document.documentElement).getPropertyValue("--border").trim() || "#e1e4e9";

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

    // Cells.
    rows.forEach((_, i) => {
      cols.forEach((_, j) => {
        const cell = cellAt(i, j);
        const x = ROW_LABEL_WIDTH + j * CELL_SIZE;
        const y = COL_LABEL_HEIGHT + i * CELL_SIZE;
        ctx.fillStyle = cell ? riskColor(cell.risk) : NO_DATA_COLOR;
        ctx.fillRect(x, y, CELL_SIZE - CELL_GAP, CELL_SIZE - CELL_GAP);
        // The ramp's safe end is the card's own surface color, so a safe
        // cell needs a visible edge or it disappears into the background.
        ctx.strokeStyle = borderColor;
        ctx.strokeRect(x + 0.5, y + 0.5, CELL_SIZE - CELL_GAP - 1, CELL_SIZE - CELL_GAP - 1);
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
  }, [rows, cols, cellAt, width, height, legendLowLabel, legendHighLabel, colorTick]);

  const onMouseMove = (e) => {
    const rect = canvasRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const col = Math.floor((x - ROW_LABEL_WIDTH) / CELL_SIZE);
    const row = Math.floor((y - COL_LABEL_HEIGHT) / CELL_SIZE);
    if (row < 0 || row >= rows.length || col < 0 || col >= cols.length) {
      setHover(null);
      return;
    }
    const cell = cellAt(row, col);
    if (!cell) {
      setHover(null);
      return;
    }
    setHover({ x: e.clientX, y: e.clientY, row, col, label: cell.label, tooltip: cell.tooltip });
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
    if (!hover || hover.label === undefined) return;
    const cx = ROW_LABEL_WIDTH + hover.col * CELL_SIZE + (CELL_SIZE - CELL_GAP) / 2;
    const cy = COL_LABEL_HEIGHT + hover.row * CELL_SIZE + (CELL_SIZE - CELL_GAP) / 2;
    const rootStyle = getComputedStyle(document.documentElement);
    const textColor = rootStyle.getPropertyValue("--text").trim() || "#1a1d24";
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
  }, [hover, width, height]);

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
          onMouseMove={onMouseMove}
          onMouseLeave={() => setHover(null)}
        />
        <canvas
          ref={overlayRef}
          className={`risk-matrix-hover-value${hover ? " visible" : ""}`}
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
