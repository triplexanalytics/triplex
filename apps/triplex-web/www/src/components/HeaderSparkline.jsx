import { useEffect, useRef, useState } from "react";
import { barsFor, binTooltip } from "../state/binTooltip";
import { WIDTH, HEIGHT } from "./HeaderSparkline.const";

/** Standalone canvas sparkline for a `ColumnDistribution` — used by the
 * Column-view tab's cards. The Table tab's grid headers draw the same bars
 * directly via Glide Data Grid's `drawHeader`, since header cells there are
 * canvas-rendered by the grid itself, not DOM nodes React can mount into;
 * this one is a plain DOM `<canvas>`, so hover uses native mouse events
 * instead of Glide's `onItemHovered`.
 *
 * ponytail: no range selection/filtering yet — clicking or dragging across
 * bars to filter the table by that range is a planned follow-up, not built
 * here. */
export default function HeaderSparkline({ distribution }) {
  const ref = useRef(null);
  const [hovered, setHovered] = useState(null);

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
      const h = (count / max) * HEIGHT;
      ctx.fillStyle = i === hovered ? "#4338ca" : "#818cf8";
      ctx.fillRect(i * barWidth, HEIGHT - h, Math.max(barWidth - 1, 1), h);
    });
  }, [distribution, hovered]);

  const barCount = barsFor(distribution).length;
  const info = hovered !== null ? binTooltip(distribution, hovered) : null;

  return (
    <div style={{ position: "relative", width: WIDTH, height: HEIGHT }}>
      <canvas
        ref={ref}
        width={WIDTH}
        height={HEIGHT}
        onMouseMove={(e) => {
          if (barCount === 0) return;
          const index = Math.min(
            Math.max(Math.floor((e.nativeEvent.offsetX / WIDTH) * barCount), 0),
            barCount - 1,
          );
          setHovered(index);
        }}
        onMouseLeave={() => setHovered(null)}
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
          <span>{info.detail}</span>
        </div>
      )}
    </div>
  );
}
