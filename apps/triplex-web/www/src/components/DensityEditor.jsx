import { useRef } from "react";

const WIDTH = 320;
const HEIGHT = 96;
const HEADROOM = 0.8; // the tallest bar fills this share of the height, leaving room to raise one

/**
 * One class's density on one number axis as a row of bars (slice T3.2). Dragging across the bars
 * redraws them; a focused bar moves with the arrow keys. Only the proportions matter: the engine
 * draws values in proportion to the weights.
 *
 * @param {{ density: number[], min: number, max: number, color: string, label: string,
 *   onChange: (bin: number, weight: number) => void }} props
 */
export default function DensityEditor({ density, min, max, color, label, onChange }) {
  const svgRef = useRef(null);
  // The scale is fixed while the pointer is down, so the bars do not rescale under the cursor.
  const scaleRef = useRef(null);
  const tallest = Math.max(...density, 0) || 1;
  const scale = scaleRef.current ?? tallest / HEADROOM;
  const barWidth = WIDTH / density.length;

  const draw = (event) => {
    const box = svgRef.current.getBoundingClientRect();
    const bin = Math.floor(((event.clientX - box.left) / box.width) * density.length);
    const fraction = 1 - (event.clientY - box.top) / box.height;
    if (bin < 0 || bin >= density.length) return;
    onChange(bin, Number((Math.min(1, Math.max(0, fraction)) * scaleRef.current).toPrecision(4)));
  };
  const start = (event) => {
    scaleRef.current = tallest / HEADROOM;
    event.currentTarget.setPointerCapture(event.pointerId);
    draw(event);
  };
  const stop = () => {
    scaleRef.current = null;
  };
  const nudge = (bin) => (event) => {
    const direction = { ArrowUp: 1, ArrowDown: -1 }[event.key];
    if (!direction) return;
    event.preventDefault();
    const step = tallest / HEADROOM / 10;
    onChange(bin, Number(Math.max(0, density[bin] + direction * step).toPrecision(4)));
  };

  return (
    <div className="density-editor">
      <svg
        ref={svgRef}
        width={WIDTH}
        height={HEIGHT}
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        role="group"
        aria-label={label}
        onPointerDown={start}
        onPointerMove={(event) => scaleRef.current !== null && draw(event)}
        onPointerUp={stop}
        onPointerCancel={stop}
      >
        {density.map((weight, bin) => {
          const height = Math.min(HEIGHT, (weight / scale) * HEIGHT);
          return (
            <rect
              key={bin}
              x={bin * barWidth + 1}
              y={HEIGHT - height}
              width={Math.max(1, barWidth - 2)}
              height={Math.max(height, 1)}
              fill={color}
              tabIndex={0}
              role="slider"
              aria-label={`Bin ${bin + 1} of ${density.length}`}
              aria-valuemin={0}
              aria-valuenow={weight}
              aria-orientation="vertical"
              onKeyDown={nudge(bin)}
            />
          );
        })}
      </svg>
      <div className="density-editor-range">
        <span>{min}</span>
        <span>{max}</span>
      </div>
    </div>
  );
}
