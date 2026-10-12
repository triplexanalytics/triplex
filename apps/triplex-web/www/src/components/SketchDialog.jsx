import { useEffect, useRef, useState } from "react";
import ParallelCoordinates from "./ParallelCoordinates";
import { classColor } from "../state/sketchEdit";

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
};

/** The classes of the sketch: pick one to steer, change its size, drop it or merge it. */
function ClassList({ s }) {
  const [mergeInto, setMergeInto] = useState("");
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
    </table>
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
          {s.sketch && (
            <span className="synth-hint">
              Proposing again replaces the sketch and your changes.
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
