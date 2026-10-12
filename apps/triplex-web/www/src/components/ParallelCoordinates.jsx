import { useMemo, useRef } from "react";
import { axisExtent, classColor } from "../state/sketchEdit";

const AXIS_GAP = 150;
const HEIGHT = 300;
const TOP = 34;
const BOTTOM = 26;
const SIDE = 70;
const MAX_LINES = 250;
const MAX_TICK_LABELS = 8;

/** Up to MAX_LINES evenly spaced rows of `sample` as one value per sketch column (a number, a
 * value index, or null), skipping `excluded` rows. */
function sampleRows(sample, columns, excluded) {
  if (!sample) return [];
  const indexOf = columns.map((c) => sample.headers.indexOf(c.name));
  const step = Math.max(1, Math.ceil(sample.rowCount / MAX_LINES));
  const rows = [];
  for (let r = 0; r < sample.rowCount; r += step) {
    if (excluded?.has(r)) continue;
    rows.push(
      columns.map((c, i) => {
        if (indexOf[i] < 0) return null;
        const text = sample.getCell(r, indexOf[i]).trim();
        if (text === "") return null;
        if (c.type === "numeric") {
          const v = Number(text);
          return Number.isFinite(v) ? v : null;
        }
        // An integer code column is a category axis whose values are number text ("3" vs "3.0").
        let k = c.values.indexOf(text);
        if (k < 0 && Number.isFinite(Number(text))) k = c.values.indexOf(String(Number(text)));
        if (k < 0) k = c.values.indexOf("(other)");
        return k < 0 ? null : k;
      }),
    );
  }
  return rows;
}

const short = (text, n = 12) => (text.length > n ? `${text.slice(0, n - 1)}…` : text);
const tick = (v) =>
  Math.abs(v) >= 1000 || Number.isInteger(v) ? Math.round(v).toLocaleString() : v.toPrecision(3);

/**
 * The Tier 3 sketch on parallel coordinates: one vertical axis per column; behind, a sample of the
 * original rows (grey) and, once generated, of the sketched rows (violet); in front, one band per
 * class joining its range on every axis. The selected class shows a handle at each end of its
 * range on number axes; dragging a handle (or using the arrow keys on it) moves that end.
 *
 * @param {{ sketch: any, original: any, generated?: any, excluded?: Set<number>,
 *   selectedClass: number, onSelectClass: (i: number) => void,
 *   onMoveRange: (classIndex: number, columnIndex: number, range: {min?: number, max?: number}) => void }} props
 */
export default function ParallelCoordinates({
  sketch,
  original,
  generated,
  excluded,
  selectedClass,
  onSelectClass,
  onMoveRange,
}) {
  const svgRef = useRef(null);
  const columns = sketch.columns;
  const originalRows = useMemo(
    () => sampleRows(original, columns, excluded),
    [original, columns, excluded],
  );
  const generatedRows = useMemo(() => sampleRows(generated, columns), [generated, columns]);
  // The axis scale covers the classes and the original data, so a class can be widened up to the
  // data's own range. It is computed from the data only, so it does not jump while dragging.
  const extents = useMemo(
    () =>
      columns.map((c, i) =>
        c.type === "numeric"
          ? axisExtent(
              sketch,
              i,
              originalRows.map((row) => row[i]),
            )
          : null,
      ),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- deliberately not `sketch`: see above
    [columns, originalRows, sketch.classes.length],
  );

  const width = SIDE * 2 + AXIS_GAP * Math.max(0, columns.length - 1);
  const x = (i) => SIDE + i * AXIS_GAP;
  const y = (i, value) => {
    const c = columns[i];
    if (c.type === "numeric") {
      const [lo, hi] = extents[i];
      return TOP + (1 - (value - lo) / (hi - lo)) * HEIGHT;
    }
    return TOP + ((value + 0.5) / c.values.length) * HEIGHT;
  };
  const valueAt = (i, clientY) => {
    const box = svgRef.current.getBoundingClientRect();
    const scale = (TOP + HEIGHT + BOTTOM) / box.height;
    const fraction = 1 - ((clientY - box.top) * scale - TOP) / HEIGHT;
    const [lo, hi] = extents[i];
    const value = lo + Math.min(1, Math.max(0, fraction)) * (hi - lo);
    return columns[i].integer ? Math.round(value) : Number(value.toPrecision(6));
  };
  const line = (row) => {
    let d = "";
    row.forEach((value, i) => {
      if (value !== null) d += `${d ? "L" : "M"}${x(i)},${y(i, value).toFixed(1)}`;
    });
    return d;
  };
  /** Top and bottom of a class on axis `i`, or null when the class is all nulls there. */
  const span = (axis, i) => {
    if (axis.null_share >= 1) return null;
    if (axis.type === "numeric") return [y(i, axis.max), y(i, axis.min)];
    const present = axis.shares.map((s, k) => (s > 0 ? k : -1)).filter((k) => k >= 0);
    if (present.length === 0) return null;
    return [y(i, present[0]) - 6, y(i, present[present.length - 1]) + 6];
  };
  const band = (cls) => {
    const spans = cls.axes.map(span);
    const top = [];
    const bottom = [];
    spans.forEach((s, i) => {
      if (!s) return;
      top.push(`${x(i)},${s[0].toFixed(1)}`);
      bottom.unshift(`${x(i)},${s[1].toFixed(1)}`);
    });
    return top.length > 1 ? `M${top.join("L")}L${bottom.join("L")}Z` : "";
  };

  const drag = (columnIndex, end) => (event) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    const move = (e) =>
      onMoveRange(selectedClass, columnIndex, { [end]: valueAt(columnIndex, e.clientY) });
    const target = event.currentTarget;
    const stop = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", stop);
      target.removeEventListener("pointercancel", stop);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", stop);
    target.addEventListener("pointercancel", stop);
  };
  const nudge = (columnIndex, end, axis) => (event) => {
    const direction = { ArrowUp: 1, ArrowDown: -1 }[event.key];
    if (!direction) return;
    event.preventDefault();
    const [lo, hi] = extents[columnIndex];
    const step = columns[columnIndex].integer
      ? Math.max(1, Math.round((hi - lo) / 50))
      : (hi - lo) / 50;
    const next = Math.min(hi, Math.max(lo, axis[end] + direction * step));
    onMoveRange(selectedClass, columnIndex, { [end]: Number(next.toPrecision(6)) });
  };

  const selected = sketch.classes[selectedClass];
  return (
    <div className="pc-scroll">
      <svg
        ref={svgRef}
        className="pc"
        width={width}
        height={TOP + HEIGHT + BOTTOM}
        viewBox={`0 0 ${width} ${TOP + HEIGHT + BOTTOM}`}
        role="img"
        aria-label="Sketch on parallel coordinates: one axis per column, one band per class"
      >
        <g className="pc-lines pc-original">
          {originalRows.map((row, r) => (
            <path key={r} d={line(row)} />
          ))}
        </g>
        <g className="pc-lines pc-generated">
          {generatedRows.map((row, r) => (
            <path key={r} d={line(row)} />
          ))}
        </g>
        {sketch.classes.map((cls, k) => (
          <path
            key={cls.name}
            className={`pc-band${k === selectedClass ? " selected" : ""}`}
            d={band(cls)}
            fill={classColor(k)}
            stroke={classColor(k)}
            onClick={() => onSelectClass(k)}
          >
            <title>{`${cls.name}: ${cls.size.toLocaleString()} rows`}</title>
          </path>
        ))}
        {columns.map((c, i) => (
          <g key={c.name} className="pc-axis">
            <line x1={x(i)} x2={x(i)} y1={TOP} y2={TOP + HEIGHT} />
            <text className="pc-axis-name" x={x(i)} y={TOP - 14} textAnchor="middle">
              <title>{c.name}</title>
              {short(c.name, 18)}
            </text>
            {c.type === "numeric" ? (
              <>
                <text x={x(i) - 12} y={TOP + 4} textAnchor="end">
                  {tick(extents[i][1])}
                </text>
                <text x={x(i) - 12} y={TOP + HEIGHT} textAnchor="end">
                  {tick(extents[i][0])}
                </text>
              </>
            ) : (
              c.values.length <= MAX_TICK_LABELS &&
              c.values.map((value, k) => (
                <text key={value} x={x(i) - 12} y={y(i, k) + 4} textAnchor="end">
                  <title>{value}</title>
                  {short(value, 9)}
                </text>
              ))
            )}
            {sketch.classes.map((cls, k) => {
              const axis = cls.axes[i];
              const offset = (k - (sketch.classes.length - 1) / 2) * 5;
              if (axis.null_share >= 1) return null;
              if (axis.type === "numeric") {
                return (
                  <line
                    key={cls.name}
                    className="pc-range"
                    x1={x(i) + offset}
                    x2={x(i) + offset}
                    y1={y(i, axis.max)}
                    y2={y(i, axis.min)}
                    stroke={classColor(k)}
                  />
                );
              }
              const total = axis.shares.reduce((a, b) => a + b, 0) || 1;
              return axis.shares.map((share, v) =>
                share > 0 ? (
                  <circle
                    key={`${cls.name}-${v}`}
                    cx={x(i) + offset}
                    cy={y(i, v)}
                    r={1.5 + 5 * Math.sqrt(share / total)}
                    fill={classColor(k)}
                  />
                ) : null,
              );
            })}
          </g>
        ))}
        {selected &&
          columns.map((c, i) => {
            const axis = selected.axes[i];
            if (c.type !== "numeric" || axis.null_share >= 1) return null;
            return ["max", "min"].map((end) => (
              <rect
                key={`${c.name}-${end}`}
                className="pc-handle"
                x={x(i) - 9}
                y={y(i, axis[end]) - 4}
                width={18}
                height={8}
                rx={2}
                fill={classColor(selectedClass)}
                tabIndex={0}
                role="slider"
                aria-label={`${selected.name}, ${c.name}: ${end === "max" ? "upper" : "lower"} end of the range`}
                aria-valuemin={extents[i][0]}
                aria-valuemax={extents[i][1]}
                aria-valuenow={axis[end]}
                aria-orientation="vertical"
                onPointerDown={drag(i, end)}
                onKeyDown={nudge(i, end, axis)}
              />
            ));
          })}
      </svg>
    </div>
  );
}
