import { useMemo, useState } from "react";
import { useJointRisk } from "../hooks/useJointRisk";
import InfoTrigger from "./InfoTrigger";
import { readRiskColorStops, reciprocalRisk } from "../state/palette";
import { kAnonymityExplanation, lDiversityDetail, tClosenessDetail, JOINT_RISK_INFO } from "../state/riskExplanations";

function weakestColumn(riskByName) {
  let weakest = null;
  for (const [name, assessment] of Object.entries(riskByName)) {
    if (assessment?.model !== "k_anonymity") continue;
    if (!weakest || assessment.value < weakest.value) weakest = { name, value: assessment.value };
  }
  return weakest;
}

/** Dataset Anonymity Analysis tab: the dataset-level verdict, below
 * DiversityCheckSection's per-pair matrices. Two reads, both fed by data
 * already on the page:
 * - "Weakest column" is `riskByName` (the same per-column k-anonymity
 *   `useColumnRisk` already computed for the Table/Column tabs) reduced to
 *   its minimum — a free, instant read, but explicitly labeled as a
 *   diagnostic, not a certification, since it never looks at columns in
 *   combination.
 * - The picker below it is the real verdict: `useJointRisk` scores the
 *   user-picked columns together as one combined quasi-identifier. See
 *   `compose_quasi_identifiers` on the Rust side for why that's a
 *   different (and correct) computation, not just an average of the
 *   per-column scores. */
export default function JointRiskSection({ sample, profile, quasiColumns, allColumns, riskByName, riskLoading }) {
  const [selected, setSelected] = useState([]);
  const [sensitiveName, setSensitiveName] = useState("");
  const { result, loading } = useJointRisk(sample, profile, selected, sensitiveName || null);
  const weakest = useMemo(() => weakestColumn(riskByName), [riskByName]);

  if (quasiColumns.length === 0) return null;

  const toggle = (name) =>
    setSelected((prev) => (prev.includes(name) ? prev.filter((n) => n !== name) : [...prev, name]));

  return (
    <div className="diversity-check-section joint-risk-section">
      <h3>
        Combined quasi-identifier check <InfoTrigger tooltip={JOINT_RISK_INFO} />
      </h3>
      <p className="diversity-check-intro">
        Pick 2 or more columns to score them together as one quasi-identifier, the way an attacker
        combining what they already know would. This is the dataset-level verdict; the bars and
        matrices above score one column (or one pair) at a time.
      </p>

      {!riskLoading && weakest && (
        <p className="joint-risk-weakest">
          Quick read: the weakest single column is <strong>{weakest.name}</strong> (k=
          {weakest.value}). Not a combined guarantee &mdash; pick columns below for that.
        </p>
      )}

      <div className="joint-risk-picker">
        <div className="joint-risk-columns" role="group" aria-label="Quasi-identifier columns">
          {quasiColumns.map((c) => (
            <label key={c.name} className="joint-risk-checkbox">
              <input type="checkbox" checked={selected.includes(c.name)} onChange={() => toggle(c.name)} />
              {c.name}
            </label>
          ))}
        </div>
        <div className="field joint-risk-sensitive">
          <label>Sensitive column (optional, for l-diversity / t-closeness)</label>
          <select value={sensitiveName} onChange={(e) => setSensitiveName(e.target.value)}>
            <option value="">None</option>
            {allColumns
              .filter((c) => !selected.includes(c.name))
              .map((c) => (
                <option key={c.name} value={c.name}>
                  {c.name}
                </option>
              ))}
          </select>
        </div>
      </div>

      {selected.length < 2 && <p className="joint-risk-hint">Pick at least 2 columns to see their combined score.</p>}
      {selected.length >= 2 && loading && <p className="joint-risk-hint">Scoring…</p>}
      {selected.length >= 2 && !loading && result && (
        <div className="joint-risk-result">
          <InfoTrigger wide tooltip={kAnonymityExplanation(result.kAnonymity.value)}>
            <div className="risk-bar-track">
              <div
                className="risk-bar-fill"
                style={{
                  width: `${reciprocalRisk(result.kAnonymity.value) * 100}%`,
                  background: readRiskColorStops().riskColor(reciprocalRisk(result.kAnonymity.value)),
                }}
              />
            </div>
          </InfoTrigger>
          <span className="risk-bar-label">
            combined k-anonymity {result.kAnonymity.value} across {selected.length} columns
          </span>
          {result.lDiversity && (
            <p className="joint-risk-hint">
              l-diversity {result.lDiversity.value}: {lDiversityDetail(result.lDiversity.value)}
            </p>
          )}
          {result.tCloseness && (
            <p className="joint-risk-hint">
              t-closeness {result.tCloseness.value.toFixed(2)}: {tClosenessDetail(result.tCloseness.value)}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
