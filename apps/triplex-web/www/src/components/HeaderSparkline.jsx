import { useEffect, useRef, useState } from "react";
import { barsFor, binFilterEntry, binTooltip } from "../state/binTooltip";
import { HISTOGRAM_BASELINE_COLOR, HISTOGRAM_COLOR, OCEAN_DRIFT } from "../state/palette";
import { WIDTH, HEIGHT } from "./HeaderSparkline.const";

/** Standalone canvas sparkline for a `ColumnDistribution` — used by the
 * Column-view tab's cards. The Table tab's grid headers draw the same bars
 * directly via Glide Data Grid's `drawHeader`, since header cells there are
 * canvas-rendered by the grid itself, not DOM nodes React can mount into;
 * this one is a plain DOM `<canvas>`, so hover uses native mouse events
 * instead of Glide's `onItemHovered`. Two separate draw implementations, by
 * necessity — but they must read fill color from palette.js (HISTOGRAM_COLOR)
 * rather than each hardcoding their own, or they silently drift apart the
 * way this one did (leftover pre-Ocean-Drift indigo, `#818cf8`/`#4338ca`,
 * missed when the header's version moved to the palette module).
 *
 * `onBinClick`, if given, turns a bar click or drag into a table row filter
 * (see useRowFilter.js): a plain click filters to that one bar, a drag
 * (mousedown on one bar, release on another — the drag can leave this small
 * canvas mid-gesture, so it's tracked with window-level listeners, not just
 * this canvas's own mouse events) filters to every bar spanned.
 *
 * `subsetCounts`, if given (useColumnSubsetDistributions.js — set whenever
 * ANY row filter is active, not just one this column originated), turns
 * every bar into two layers: its full unfiltered height in
 * HISTOGRAM_BASELINE_COLOR, plus a shorter HISTOGRAM_COLOR overlay sized to
 * how many of that bar's own rows survive the filter. Coordinated-view
 * brushing, one mechanism for both roles: on the column that originated a
 * sparkline-driven filter, a selected bar's subset count equals its full
 * count (overlay fills it completely) and an unselected bar's is 0 (no
 * overlay, reads as grayed out) — no separate "am I the source card" code
 * path needed. On every other column it reads as the usual filtered/
 * unfiltered overlay. */
export default function HeaderSparkline({ distribution, columnName, onBinClick, subsetCounts }) {
  const ref = useRef(null);
  const [hovered, setHovered] = useState(null);
  const [dragRange, setDragRange] = useState(null); // [startIndex, currentIndex] | null
  const dragStartRef = useRef(null);

  const barCount = barsFor(distribution).length;
  const indexAt = (offsetX) =>
    Math.min(Math.max(Math.floor((offsetX / WIDTH) * barCount), 0), barCount - 1);
  const activeRange = dragRange && [Math.min(...dragRange), Math.max(...dragRange)];

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    ctx.clearRect(0, 0, WIDTH, HEIGHT);
    if (!distribution) return;

    const bars = barsFor(distribution);
    if (bars.length === 0) return;

    const max = Math.max(...bars);
    if (max === 0) return;
    const barWidth = WIDTH / bars.length;
    bars.forEach((count, i) => {
      const active = activeRange ? i >= activeRange[0] && i <= activeRange[1] : i === hovered;
      const w = Math.max(barWidth - 1, 1);
      const barX = i * barWidth;
      if (subsetCounts) {
        const h = (count / max) * HEIGHT;
        ctx.fillStyle = HISTOGRAM_BASELINE_COLOR;
        ctx.fillRect(barX, HEIGHT - h, w, h);
        const subsetH = ((subsetCounts[i] ?? 0) / max) * HEIGHT;
        if (subsetH > 0) {
          ctx.fillStyle = active ? OCEAN_DRIFT.primary : HISTOGRAM_COLOR;
          ctx.fillRect(barX, HEIGHT - subsetH, w, subsetH);
        }
      } else {
        const h = (count / max) * HEIGHT;
        ctx.fillStyle = active ? OCEAN_DRIFT.primary : HISTOGRAM_COLOR;
        ctx.fillRect(barX, HEIGHT - h, w, h);
      }
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- activeRange is
    // derived from dragRange, listed separately here only for clarity.
  }, [distribution, hovered, dragRange, subsetCounts]);

  const rangeSummary = () => {
    if (!activeRange || activeRange[0] === activeRange[1]) return null;
    const bars = barsFor(distribution);
    const total = bars.slice(activeRange[0], activeRange[1] + 1).reduce((a, b) => a + b, 0);
    return {
      title: `${activeRange[1] - activeRange[0] + 1} bars`,
      detail: `${total} value${total === 1 ? "" : "s"} — release to filter`,
    };
  };
  const hoverInfo = hovered !== null ? binTooltip(distribution, hovered) : null;
  if (hoverInfo && subsetCounts) {
    hoverInfo.detail += ` (${subsetCounts[hovered] ?? 0} in current selection)`;
  }
  const info = rangeSummary() ?? hoverInfo;

  return (
    <div style={{ position: "relative", width: WIDTH, height: HEIGHT }}>
      <canvas
        ref={ref}
        width={WIDTH}
        height={HEIGHT}
        style={onBinClick ? { cursor: "crosshair" } : undefined}
        onMouseMove={(e) => {
          if (barCount === 0) return;
          setHovered(indexAt(e.nativeEvent.offsetX));
        }}
        onMouseLeave={() => setHovered(null)}
        onClick={(e) => {
          // Filtering itself happens on mouseup below (covers both a plain
          // click and a drag); this only stops the click from also bubbling
          // up into ColumnCard's own onClick (card selection).
          if (onBinClick) e.stopPropagation();
        }}
        onMouseDown={(e) => {
          if (!onBinClick || barCount === 0) return;
          e.stopPropagation();
          const canvas = ref.current;
          const start = indexAt(e.nativeEvent.offsetX);
          dragStartRef.current = start;
          setDragRange([start, start]);

          const handleMove = (moveEvent) => {
            const rect = canvas.getBoundingClientRect();
            setDragRange([dragStartRef.current, indexAt(moveEvent.clientX - rect.left)]);
          };
          const handleUp = (upEvent) => {
            window.removeEventListener("mousemove", handleMove);
            window.removeEventListener("mouseup", handleUp);
            const rect = canvas.getBoundingClientRect();
            const end = indexAt(upEvent.clientX - rect.left);
            const entry = binFilterEntry(columnName, distribution, dragStartRef.current, end);
            dragStartRef.current = null;
            setDragRange(null);
            if (entry) onBinClick([entry]);
          };
          window.addEventListener("mousemove", handleMove);
          window.addEventListener("mouseup", handleUp);
        }}
      />
      {info && (
        <div
          className="bin-tooltip"
          style={{
            position: "absolute",
            left: WIDTH / 2,
            top: 0,
            transform: "translate(-50%, -100%)",
          }}
        >
          <strong>{info.title}</strong>
          <span>{info.detail}{onBinClick && !activeRange ? " — click or drag to filter" : ""}</span>
        </div>
      )}
    </div>
  );
}
