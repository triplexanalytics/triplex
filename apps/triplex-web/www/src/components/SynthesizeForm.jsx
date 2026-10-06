import { needsPublicConfirmation } from "../state/synthesisConfig";

/**
 * @param {{ label: string, hint?: string, error?: unknown, children: import("react").ReactNode }} props
 */
function Field({ label, hint, error, children }) {
  return (
    <label className={`synth-field${error ? " has-error" : ""}`}>
      <span className="synth-field-label">{label}</span>
      {children}
      {hint && <span className="synth-hint">{hint}</span>}
    </label>
  );
}

function DomainRow({ column, onEdit }) {
  const fromData = column.include && column.source === "suggested";
  return (
    <tr className={column.include ? "" : "synth-row-off"}>
      <td>
        <input
          type="checkbox"
          checked={column.include}
          onChange={(e) => onEdit(column.name, { include: e.target.checked })}
          aria-label={`Synthesize ${column.name}`}
        />
      </td>
      <td>
        {column.name}
        {column.note && <span className="synth-note"> ({column.note})</span>}
      </td>
      <td>
        <select
          value={column.kind}
          onChange={(e) => onEdit(column.name, { kind: e.target.value })}
          aria-label={`${column.name} type`}
        >
          <option value="numeric">number</option>
          <option value="categorical">category</option>
        </select>
      </td>
      {column.kind === "numeric" ? (
        <>
          <td>
            <input
              type="text"
              inputMode="decimal"
              value={column.min}
              onChange={(e) => onEdit(column.name, { min: e.target.value })}
              aria-label={`${column.name} minimum`}
            />
          </td>
          <td>
            <input
              type="text"
              inputMode="decimal"
              value={column.max}
              onChange={(e) => onEdit(column.name, { max: e.target.value })}
              aria-label={`${column.name} maximum`}
            />
          </td>
          <td>
            <input
              type="checkbox"
              checked={column.integer}
              onChange={(e) => onEdit(column.name, { integer: e.target.checked })}
              aria-label={`${column.name} whole numbers only`}
            />
          </td>
        </>
      ) : (
        <td colSpan={3}>
          <input
            type="text"
            className="synth-values"
            value={column.values}
            placeholder="value one, value two, …"
            onChange={(e) => onEdit(column.name, { values: e.target.value })}
            aria-label={`${column.name} category values`}
          />
        </td>
      )}
      <td>{fromData && <span className="synth-badge">from your data</span>}</td>
    </tr>
  );
}

/** The Tier 2 form, shown inside SynthesizeDialog. Everything runs in this browser. `onRun`
 * starts a run and is expected to close the dialog when it succeeds. */
export default function SynthesizeForm({ synthesis, hasFile, fileName, onRun }) {
  const s = synthesis;
  const isDp = s.mode === "dp";
  const errorFor = (field) => s.validation.errors.find((e) => e.field === field);
  const needsConfirm = isDp && needsPublicConfirmation(s.columns);

  if (!hasFile) {
    return <p className="synth-hint">Load a file to generate synthetic data from it.</p>;
  }

  return (
    <div className="synth-tab">
      <p className="synth-intro">
        Generates a new table with the same statistical shape as <strong>{fileName}</strong>.
        Nothing leaves your browser.
      </p>

      <fieldset className="synth-modes">
        <legend>Kind of synthetic data</legend>
        <label>
          <input
            type="radio"
            name="synth-mode"
            checked={!isDp}
            onChange={() => s.setMode("standard")}
          />{" "}
          Standard <span className="synth-hint">(no privacy guarantee)</span>
        </label>
        <label>
          <input type="radio" name="synth-mode" checked={isDp} onChange={() => s.setMode("dp")} />{" "}
          Differentially private{" "}
          <span className="synth-hint">(formal guarantee, needs a budget)</span>
        </label>
      </fieldset>

      {isDp ? (
        <div className="banner banner-warning" role="note">
          You choose the columns and their <strong>public</strong> bounds; nothing is taken from the
          data unless you ask, and then you must confirm the values are public knowledge. Bounds
          read from the data would themselves reveal information and void the guarantee.
        </div>
      ) : (
        <div className="banner banner-warning" role="note">
          <strong>No formal privacy guarantee.</strong> The report measures how close the output is
          to your data and how many rows it copies, but a good number does not certify privacy. Do
          not treat the output as anonymous. Choose “Differentially private” for a formal guarantee.
        </div>
      )}

      {s.rowCount !== null && (
        <p className="synth-hint">
          Uses {s.rowCount.toLocaleString()} rows. Rows and columns you removed in the Table are not
          used.
        </p>
      )}

      <div className="synth-fields">
        <Field label="Seed" hint="Makes the sampling repeatable." error={errorFor("seed")}>
          <input
            type="text"
            inputMode="numeric"
            value={s.seed}
            onChange={(e) => s.setSeed(e.target.value)}
          />
        </Field>
        <Field
          label="Rows to generate"
          hint="Blank = as many as the original."
          error={errorFor("rowsOut")}
        >
          <input
            type="text"
            inputMode="numeric"
            value={s.rowsOut}
            onChange={(e) => s.setRowsOut(e.target.value)}
          />
        </Field>
        {isDp ? (
          <Field
            label="Privacy budget ε (required)"
            hint="Smaller is more private and less accurate. There is no default."
            error={errorFor("epsilon")}
          >
            <input
              type="text"
              inputMode="decimal"
              value={s.epsilon}
              placeholder="e.g. 1"
              onChange={(e) => s.setEpsilon(e.target.value)}
            />
          </Field>
        ) : (
          <Field label="Merge categories seen fewer than" hint="times into “(other)”.">
            <input
              type="text"
              inputMode="numeric"
              value={s.minCategoryCount}
              onChange={(e) => s.setMinCategoryCount(e.target.value)}
            />
          </Field>
        )}
      </div>

      {isDp && (
        <>
          <details className="synth-advanced">
            <summary>Advanced</summary>
            <div className="synth-fields">
              <Field label="Share of ε for column distributions" error={errorFor("marginalShare")}>
                <input
                  type="text"
                  inputMode="decimal"
                  value={s.marginalShare}
                  onChange={(e) => s.setMarginalShare(e.target.value)}
                />
              </Field>
              <Field label="Histogram bins per number column" error={errorFor("bins")}>
                <input
                  type="text"
                  inputMode="numeric"
                  value={s.bins}
                  onChange={(e) => s.setBins(e.target.value)}
                />
              </Field>
            </div>
          </details>

          <div className="synth-domain-head">
            <h4>Columns and their public bounds</h4>
            <button type="button" onClick={s.suggest} disabled={s.suggesting}>
              {s.suggesting ? "Reading…" : "Suggest from my data"}
            </button>
          </div>
          <p className="synth-hint">
            Suggestions are read from your data, so they are <em>not</em> public bounds until you
            confirm them (or type your own).
          </p>
          <table className="synth-table synth-domain-table">
            <thead>
              <tr>
                <th />
                <th>Column</th>
                <th>Type</th>
                <th>Min</th>
                <th>Max</th>
                <th title="Whole numbers only">Int</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {s.columns.map((column) => (
                <DomainRow key={column.name} column={column} onEdit={s.editColumn} />
              ))}
            </tbody>
          </table>
          {errorFor("columns") && <p className="synth-error">{errorFor("columns").message}</p>}
          {s.validation.errors
            .filter((e) => e.field.startsWith("column:") || e.field === "rows")
            .map((e) => (
              <p key={e.field} className="synth-error">
                {e.message}
              </p>
            ))}

          {needsConfirm && (
            <label className="synth-confirm">
              <input
                type="checkbox"
                checked={s.confirmedPublic}
                onChange={(e) => s.setConfirmedPublic(e.target.checked)}
              />{" "}
              The bounds marked “from your data” are <strong>public knowledge</strong>, not derived
              from this data.
            </label>
          )}

          <label className="synth-confirm">
            <input
              type="checkbox"
              checked={s.includeDiagnostics}
              onChange={(e) => s.setIncludeDiagnostics(e.target.checked)}
            />{" "}
            Also show diagnostics computed from my data{" "}
            <span className="synth-hint">
              (how close the output is, and how many rows are copies). These are{" "}
              <strong>not covered by the privacy guarantee</strong>.
            </span>
          </label>
        </>
      )}

      <div className="synth-run">
        <button
          type="button"
          className="primary"
          onClick={onRun}
          disabled={!s.validation.ok || s.running}
        >
          {s.running ? "Generating…" : "Generate synthetic data"}
        </button>
        {!s.validation.ok && (
          <ul className="synth-missing" aria-label="What is still needed">
            {s.validation.errors.map((e) => (
              <li key={`${e.field}-${e.message}`}>{e.message}</li>
            ))}
          </ul>
        )}
      </div>

      {s.error && <div className="banner banner-error">{s.error}</div>}
    </div>
  );
}
