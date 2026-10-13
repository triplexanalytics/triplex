import { useEffect, useRef, useState } from "react";
import ParallelCoordinates from "./ParallelCoordinates";
import DensityEditor from "./DensityEditor";
import {
  DEFAULT_MIN_CLASS_SIZE,
  classColor,
  cutProblem,
  splittableColumns,
} from "../state/sketchEdit";

/** @param {{ label: string, hint?: string, error?: string, children: any }} props */
function Field({ label, hint, error, children }) {
  return (
    <label className={`synth-field${error ? " has-error" : ""}`}>
      <span className="synth-field-label">{label}</span>
      {children}
      {hint && <span className="synth-hint">{hint}</span>}
    </label>
  );
}

const BUSY_LABEL = {
  proposing: "Proposing a sketch…",
  merging: "Merging…",
  generating: "Generating…",
  splitting: "Splitting…",
  adding: "Adding…",
  loading: "Loading…",
};

/** The classes of the sketch: pick one to steer, change its size, drop it or merge it, or add a
 * new one by hand. */
function ClassList({ s }) {
  const [mergeInto, setMergeInto] = useState("");
  const [newSize, setNewSize] = useState(String(DEFAULT_MIN_CLASS_SIZE));
  const newSizeOk = /^\d+$/.test(newSize.trim());
  const { sketch, selectedClass } = s;
  const others = sketch.classes.map((c, i) => ({ c, i })).filter(({ i }) => i !== selectedClass);
  return (
    <table className="synth-table sketch-classes">
      <thead>
        <tr>
          <th>Class</th>
          <th title="How many rows this class stands for. Classes are drawn in proportion to it.">
            Rows
          </th>
          <th />
        </tr>
      </thead>
      <tbody>
        {sketch.classes.map((cls, k) => (
          <tr key={cls.name} className={k === selectedClass ? "selected" : ""}>
            <td>
              <label>
                <input
                  type="radio"
                  name="sketch-class"
                  checked={k === selectedClass}
                  onChange={() => s.setSelectedClass(k)}
                />{" "}
                <span className="sketch-swatch" style={{ background: classColor(k) }} />
                {cls.name}
              </label>
            </td>
            <td>
              <input
                type="number"
                min="0"
                step="1"
                value={cls.size}
                aria-label={`Rows in ${cls.name}`}
                onChange={(e) => s.resizeClass(k, Number(e.target.value))}
              />
            </td>
            <td>
              {k === selectedClass && sketch.classes.length > 1 && (
                <span className="sketch-class-actions">
                  <button type="button" onClick={() => s.removeClass(k)}>
                    Drop
                  </button>
                  <select
                    value={mergeInto}
                    aria-label={`Merge ${cls.name} with another class`}
                    onChange={(e) => setMergeInto(e.target.value)}
                  >
                    <option value="">Merge with…</option>
                    {others.map(({ c, i }) => (
                      <option key={c.name} value={i}>
                        {c.name}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    disabled={mergeInto === "" || !!s.busy}
                    onClick={async () => {
                      await s.merge(k, Number(mergeInto));
                      setMergeInto("");
                    }}
                  >
                    Merge
                  </button>
                </span>
              )}
            </td>
          </tr>
        ))}
      </tbody>
      <tfoot>
        <tr>
          <td>New class</td>
          <td>
            <input
              type="number"
              min="0"
              step="1"
              value={newSize}
              aria-label="Rows in the new class"
              onChange={(e) => setNewSize(e.target.value)}
            />
          </td>
          <td>
            <button
              type="button"
              disabled={!newSizeOk || !!s.busy}
              title="Adds a class as wide as the whole sketch, for you to narrow"
              onClick={() => s.addClass(Number(newSize))}
            >
              Add a class
            </button>
          </td>
        </tr>
      </tfoot>
    </table>
  );
}

/** Split the selected class in two along a column, and redraw its density on a number column. */
function ShapePanel({ s }) {
  const { sketch, selectedClass } = s;
  const cls = sketch.classes[selectedClass];
  const splittable = splittableColumns(sketch, selectedClass);
  const [splitColumn, setSplitColumn] = useState("");
  const [cut, setCut] = useState("");
  const [shapeColumn, setShapeColumn] = useState("");
  if (!cls) return null;

  const chosen = splittable.find((c) => String(c.index) === splitColumn) ?? splittable[0];
  const chosenAxis = chosen ? cls.axes[chosen.index] : null;
  const cutValue =
    chosen?.type === "numeric"
      ? cut === ""
        ? String(chosen.at)
        : cut
      : cut || String(chosen?.values[0].position ?? "");
  const problem = chosen?.type === "numeric" ? cutProblem(chosenAxis, cutValue) : null;
  const doSplit = async () => {
    const done = await s.split(
      selectedClass,
      chosen.index,
      chosen.type === "numeric" ? { at: Number(cutValue) } : { values: [Number(cutValue)] },
    );
    if (done) setCut("");
  };

  const shapeable = sketch.columns
    .map((column, i) => ({ column, axis: cls.axes[i], i }))
    .filter(({ axis }) => axis.type === "numeric" && axis.null_share < 1 && axis.min < axis.max);
  const shaped = shapeable.find(({ i }) => String(i) === shapeColumn) ?? shapeable[0];

  return (
    <div className="sketch-shape">
      <fieldset>
        <legend>Split {cls.name}</legend>
        {chosen ? (
          <>
            <div className="sketch-row">
              <select
                value={String(chosen.index)}
                aria-label="Column to split along"
                onChange={(e) => {
                  setSplitColumn(e.target.value);
                  setCut("");
                }}
              >
                {splittable.map((c) => (
                  <option key={c.index} value={c.index}>
                    {c.name}
                  </option>
                ))}
              </select>
              {chosen.type === "numeric" ? (
                <input
                  type="number"
                  step="any"
                  value={cutValue}
                  aria-label="Value to cut at"
                  onChange={(e) => setCut(e.target.value)}
                />
              ) : (
                <select
                  value={cutValue}
                  aria-label="Value to split off"
                  onChange={(e) => setCut(e.target.value)}
                >
                  {chosen.values.map((v) => (
                    <option key={v.position} value={v.position}>
                      {v.label}
                    </option>
                  ))}
                </select>
              )}
              <button type="button" disabled={!!problem || !!s.busy} onClick={doSplit}>
                Split
              </button>
            </div>
            <p className="synth-hint">
              {problem ??
                (chosen.type === "numeric"
                  ? "Two classes: below and above the cut. The data generated stays the same until you move one of them."
                  : "Two classes: this value, and the rest. The data generated stays the same until you change one of them.")}
            </p>
          </>
        ) : (
          <p className="synth-hint">This class has nothing to split along.</p>
        )}
      </fieldset>
      {shaped && (
        <fieldset>
          <legend>Shape of {cls.name}</legend>
          <div className="sketch-row">
            <select
              value={String(shaped.i)}
              aria-label="Column whose shape to redraw"
              onChange={(e) => setShapeColumn(e.target.value)}
            >
              {shapeable.map(({ column, i }) => (
                <option key={i} value={i}>
                  {column.name}
                </option>
              ))}
            </select>
            <button type="button" onClick={() => s.flatten(selectedClass, shaped.i)}>
              Make flat
            </button>
          </div>
          <DensityEditor
            density={shaped.axis.density}
            min={shaped.axis.min}
            max={shaped.axis.max}
            color={classColor(selectedClass)}
            label={`How ${shaped.column.name} is spread inside ${cls.name}: drag across the bars to redraw`}
            onChange={(bin, weight) => s.redraw(selectedClass, shaped.i, bin, weight)}
          />
          <p className="synth-hint">
            Drag across the bars to redraw how values are spread over the class&apos;s range.
          </p>
        </fieldset>
      )}
    </div>
  );
}

/** The selected class's range on every number column, as typed values (the same edit as dragging
 * a handle in the view). */
function RangeTable({ s }) {
  const { sketch, selectedClass } = s;
  const cls = sketch.classes[selectedClass];
  if (!cls) return null;
  const rows = sketch.columns
    .map((column, i) => ({ column, axis: cls.axes[i], i }))
    .filter(({ axis }) => axis.type === "numeric" && axis.null_share < 1);
  if (rows.length === 0) return null;
  const input = (i, end, axis, name) => (
    <input
      type="number"
      step="any"
      value={axis[end]}
      aria-label={`${cls.name}, ${name}: ${end === "min" ? "lowest" : "highest"} value`}
      onChange={(e) =>
        e.target.value !== "" && s.moveRange(selectedClass, i, { [end]: Number(e.target.value) })
      }
    />
  );
  return (
    <table className="synth-table sketch-ranges">
      <thead>
        <tr>
          <th>Range of {cls.name}</th>
          <th>From</th>
          <th>To</th>
        </tr>
      </thead>
      <tbody>
        {rows.map(({ column, axis, i }) => (
          <tr key={column.name}>
            <td>{column.name}</td>
            <td>{input(i, "min", axis, column.name)}</td>
            <td>{input(i, "max", axis, column.name)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Tier 3 (sketch-based synthesis) as a modal over the table: propose a sketch from the data,
 * steer it on parallel coordinates, generate. Uses the native <dialog> like SynthesizeDialog. */
export default function SketchDialog({
  sketching: s,
  original,
  excluded,
  fileName,
  onClose,
  onDone,
}) {
  const ref = useRef(null);
  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
  }, []);
  const errorFor = (field) => s.settingErrors.find((e) => e.field === field)?.message;
  const generate = async () => {
    if (await s.generate()) onDone();
  };
  const blocked = s.settingErrors.length > 0;

  return (
    <dialog
      ref={ref}
      className="synth-dialog sketch-dialog"
      onClose={onClose}
      aria-label="Sketch data"
    >
      <div className="synth-dialog-head">
        <h3>Sketch data</h3>
        <button type="button" onClick={() => ref.current?.close()} aria-label="Close">
          Close
        </button>
      </div>
      <div className="synth-dialog-body">
        <div className="banner banner-warning" role="note">
          <strong>No privacy guarantee.</strong> Sketched data is generated from a description of
          your data, not from a privacy mechanism. The sketch itself is derived from your data (its
          ranges end at real values): treat it like the data.
        </div>
        <p className="synth-hint">
          {fileName ? `${fileName}: ` : ""}the rows are grouped into classes. Each class has a range
          on every number column and a share for every category. New rows are drawn class by class,
          every column on its own inside a class, so what links columns is the mix of classes.
          Nothing leaves this browser.
        </p>
        <div className="synth-fields">
          <Field label="Classes" error={errorFor("classes")}>
            <input
              type="text"
              inputMode="numeric"
              value={s.classes}
              onChange={(e) => s.setClasses(e.target.value)}
            />
          </Field>
          <Field
            label="Minimum class size"
            hint="rows. A smaller class is folded into the nearest one."
            error={errorFor("minClassSize")}
          >
            <input
              type="text"
              inputMode="numeric"
              value={s.minClassSize}
              onChange={(e) => s.setMinClassSize(e.target.value)}
            />
          </Field>
          <Field label="Density bins per number column" error={errorFor("bins")}>
            <input
              type="text"
              inputMode="numeric"
              value={s.bins}
              onChange={(e) => s.setBins(e.target.value)}
            />
          </Field>
          <Field label="Seed" error={errorFor("seed")}>
            <input
              type="text"
              inputMode="numeric"
              value={s.seed}
              onChange={(e) => s.setSeed(e.target.value)}
            />
          </Field>
          <Field
            label="Rows to generate"
            hint="Blank: as many as the data."
            error={errorFor("rowsOut")}
          >
            <input
              type="text"
              inputMode="numeric"
              value={s.rowsOut}
              onChange={(e) => s.setRowsOut(e.target.value)}
            />
          </Field>
        </div>
        {blocked && (
          <ul className="synth-missing">
            {s.settingErrors.map((e) => (
              <li key={e.field}>{e.message}</li>
            ))}
          </ul>
        )}
        <div className="synth-run">
          <button type="button" onClick={s.propose} disabled={blocked || !!s.busy}>
            {s.sketch ? "Propose again" : "Propose a sketch"}
          </button>
          <label className="sketch-load">
            Load a sketch (.json)
            <input
              type="file"
              accept=".json,application/json"
              disabled={!!s.busy}
              onChange={(e) => {
                const picked = e.target.files?.[0];
                if (picked) s.loadSketch(picked);
                e.target.value = "";
              }}
            />
          </label>
          {s.sketch && (
            <span className="synth-hint">
              Proposing again or loading a sketch replaces the sketch and your changes.
            </span>
          )}
        </div>
        {s.error && <div className="banner banner-error synth-error">{s.error}</div>}

        {s.sketch && (
          <>
            <p className="synth-hint">
              Grey lines are a sample of your rows
              {s.result ? ", violet lines a sample of the sketched rows" : ""}; each colored band is
              a class. Pick a class, then drag the handles on an axis (or type a range below) to
              move or narrow it.
            </p>
            <ParallelCoordinates
              sketch={s.sketch}
              original={original}
              generated={s.result?.sample}
              excluded={excluded}
              selectedClass={s.selectedClass}
              onSelectClass={s.setSelectedClass}
              onMoveRange={s.moveRange}
            />
            <div className="sketch-panels">
              <ClassList s={s} />
              <RangeTable s={s} />
              <ShapePanel key={s.sketch.classes[s.selectedClass]?.name} s={s} />
            </div>
            <div className="synth-run">
              <button
                type="button"
                className="primary"
                onClick={generate}
                disabled={blocked || !!s.problem || !!s.busy}
              >
                {s.busy ? BUSY_LABEL[s.busy] : "Generate sketched data"}
              </button>
              <button type="button" onClick={s.downloadSketch}>
                Download sketch (.json)
              </button>
              {s.problem && <span className="synth-hint">{s.problem}</span>}
            </div>
          </>
        )}
      </div>
    </dialog>
  );
}
