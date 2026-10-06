const pct = (x) => (x == null ? "n/a" : `${(x * 100).toFixed(1)}%`);
const num = (x, digits = 4) => (x == null ? "n/a" : Number(x).toFixed(digits));

function Notes({ title, items }) {
  if (!items || items.length === 0) return null;
  return (
    <details className="synth-notes">
      <summary>{title}</summary>
      <ul>
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </details>
  );
}

function FidelityTable({ fidelity }) {
  return (
    <>
      <table className="synth-table">
        <thead>
          <tr>
            <th>Column</th>
            <th>Kind</th>
            <th title="0 = identical distributions, 1 = completely different">Difference</th>
            <th>Missing (original / synthetic)</th>
          </tr>
        </thead>
        <tbody>
          {fidelity.columns.map((c) => (
            <tr key={c.column}>
              <td>{c.column}</td>
              <td>{c.kind}</td>
              <td>{num(c.kind === "numeric" ? c.ks_statistic : c.total_variation)}</td>
              <td>
                {pct(c.null_rate_real)} / {pct(c.null_rate_synthetic)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="synth-hint">
        Difference is the Kolmogorov-Smirnov distance for numbers and the total variation distance
        for categories: 0 means the distributions match. Numeric correlation:{" "}
        {fidelity.correlation.pairs_compared} pair(s) compared, largest error{" "}
        {num(fidelity.correlation.max_abs_error, 3)}, average{" "}
        {num(fidelity.correlation.mean_abs_error, 3)}. {fidelity.correlation.note}.
      </p>
    </>
  );
}

function Leakage({ leakage, rowsOut }) {
  return (
    <p className="synth-hint">
      Rows identical to an original row: <strong>{leakage.exact_copy_rows}</strong> of{" "}
      {rowsOut.toLocaleString()} ({pct(leakage.exact_copy_rate)}). Nearest-original distance ratio:{" "}
      <strong>{num(leakage.dcr_ratio, 2)}</strong> (near 1 is what independent data looks like; well
      below 1 means synthetic rows sit closer to originals than originals do to each other).
      Measured, not certified. {leakage.note}
    </p>
  );
}

function StandardResult({ report }) {
  return (
    <>
      <h4>How close is it to your data?</h4>
      <FidelityTable fidelity={report.fidelity} />
      <h4>Copying</h4>
      <Leakage leakage={report.leakage} rowsOut={report.rows_out} />
      <div className="banner banner-warning" role="note">
        {report.privacy_statement}
      </div>
      {report.columns_dropped.length > 0 && (
        <>
          <h4>Not synthesized</h4>
          <ul className="synth-list">
            {report.columns_dropped.map((d) => (
              <li key={d.column}>
                <strong>{d.column}</strong>: {d.reason}
              </li>
            ))}
          </ul>
        </>
      )}
      {report.categories_merged.length > 0 && (
        <p className="synth-hint">
          Rare categories merged into “(other)”:{" "}
          {report.categories_merged
            .map((m) => `${m.column} (${m.categories_merged} categories, ${m.rows_affected} rows)`)
            .join("; ")}
          .
        </p>
      )}
      <Notes title="Limitations" items={report.limitations} />
    </>
  );
}

function DpResult({ report, diagnosticsIncluded }) {
  const p = report.privacy;
  const diagnostics = report.nonprivate_diagnostics;
  return (
    <>
      <div className="banner banner-success" role="note">
        <strong>
          {p.guarantee}. Total budget ε = {p.epsilon_total}.
        </strong>{" "}
        {p.statement}
      </div>
      <ul className="synth-list">
        <li>Neighbouring datasets differ by: {p.neighbor_relation}.</li>
        <li>Number of rows treated as public: {p.row_count_public ? "yes" : "no"}.</li>
        <li>
          Column list and bounds treated as public: {p.bounds_and_columns_public ? "yes" : "no"}.
        </li>
        <li>Noise source: {p.noise_source}.</li>
        <li>{p.epsilon_rounding}.</li>
      </ul>
      <h4>Privacy budget</h4>
      <p className="synth-hint">
        Spent {num(p.epsilon_spent, 6)} of {p.epsilon_total}.
      </p>
      <table className="synth-table">
        <thead>
          <tr>
            <th>Step</th>
            <th>Sensitivity</th>
            <th>ε allotted</th>
            <th>ε used</th>
          </tr>
        </thead>
        <tbody>
          {p.ledger.map((s) => (
            <tr key={s.mechanism}>
              <td>{s.mechanism}</td>
              <td>{s.sensitivity.toLocaleString()}</td>
              <td>{num(s.epsilon_allotted, 6)}</td>
              <td>{num(s.epsilon_used, 6)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <Notes title="Warnings" items={report.warnings} />
      <Notes title="Limitations" items={report.limitations} />
      {diagnostics && (
        <section className="synth-diagnostics" aria-label="Non-private diagnostics">
          <div className="banner banner-error" role="alert">
            {diagnostics.warning}
          </div>
          <h4>Non-private diagnostics</h4>
          <FidelityTable fidelity={diagnostics.fidelity} />
          <Leakage leakage={diagnostics.leakage} rowsOut={report.rows_out} />
        </section>
      )}
      {diagnosticsIncluded && (
        <p className="synth-hint">
          The downloaded report includes these non-private figures. Do not share it with anyone who
          should not see the original data.
        </p>
      )}
    </>
  );
}

/** The report for the one synthesis run behind the Synthetic dataset: the first thing in the
 * Privacy Analysis (Synthetic Data) tab. Downloads live in the navbar's Download menu. */
export default function SynthesisReport({ result }) {
  const { report, mode, diagnosticsIncluded } = result;
  return (
    <section className="synth-result" aria-label="Synthesis report">
      <p className="synth-hint">
        {report.rows_out.toLocaleString()} synthetic rows from {report.rows_in.toLocaleString()}{" "}
        original rows
        {report.rows_excluded > 0 ? ` (${report.rows_excluded} removed rows not used)` : ""}
      </p>
      {mode === "dp" ? (
        <DpResult report={report} diagnosticsIncluded={diagnosticsIncluded} />
      ) : (
        <StandardResult report={report} />
      )}
    </section>
  );
}
