import { useMemo } from "react";
import InfoTrigger from "./InfoTrigger";
import { readRiskColorStops } from "../state/palette";
import { rowRiskInfo, rowRiskDetail } from "../state/riskExplanations";

const TOP_N = 10;
const MODEL_LABEL = { suda: "SUDA", ebm: "EBM" };

/** Dataset Anonymity Analysis tab: the one row-level (not column or
 * column-pair) risk view in the app — every matrix above answers "how
 * safe is this column/pair," this answers "which specific rows are most
 * identifiable" (SUDA/DIS-SUDA or EBM — `model`
 * picks which, `onModelChange` switches it). Ranks the riskiest rows by
 * score, capped to the top `TOP_N` so a large file doesn't dump a score
 * for every single row; rows scoring 0 or below are left out entirely
 * rather than padding the list down to `TOP_N` with rows that carry no
 * signal (SUDA's 0 has an exact meaning, "never part of any minimal
 * unique combination"; EBM rarely lands exactly on 0, so this filter is
 * mostly inert for it, which is fine — nothing above it needs a
 * model-specific cutoff).
 *
 * Shows each row by its 1-based position (still useful to cross-reference
 * the Table tab) plus its actual value on every quasi-identifier column —
 * not the candidate sensitive column's value, since both techniques score
 * off the quasi-identifier columns alone, so those are what explain *why*
 * the row stood out. */
export default function RowRiskSection({
  raw,
  loading,
  quasiColumns,
  sample,
  model,
  onModelChange,
}) {
  const ranked = useMemo(() => {
    if (!raw?.rowRisk) return null;
    return raw.rowRisk
      .map(([score, share], index) => ({ index, score, share }))
      .filter((row) => row.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, TOP_N);
  }, [raw]);

  const colIndex = useMemo(() => {
    if (!sample) return {};
    const map = {};
    sample.headers.forEach((name, i) => (map[name] = i));
    return map;
  }, [sample]);

  if (quasiColumns.length === 0) return null;

  const modelLabel = MODEL_LABEL[model] ?? MODEL_LABEL.suda;

  return (
    <div className="row-risk-section">
      <div className="row-risk-header">
        <h3>
          Row risk ({modelLabel}) <InfoTrigger tooltip={rowRiskInfo(model)} align="left" />
        </h3>
        <select
          className="row-risk-model-select"
          value={model ?? "suda"}
          onChange={(e) => onModelChange(e.target.value)}
          aria-label="Row risk technique"
        >
          <option value="suda">SUDA</option>
          <option value="ebm">EBM</option>
        </select>
      </div>
      {loading || !raw ? (
        <p className="row-risk-empty">Scoring rows…</p>
      ) : !raw.rowRisk ? (
        <p className="row-risk-empty">
          Not shown for more than 10 quasi-identifier columns at once (cost grows exponentially with
          column count for SUDA) — this file has {quasiColumns.length}.
        </p>
      ) : ranked.length === 0 ? (
        <p className="row-risk-empty">
          No row scores above 0 by {modelLabel} across the {quasiColumns.length} quasi-identifier
          column{quasiColumns.length === 1 ? "" : "s"} checked.
        </p>
      ) : (
        <table className="row-risk-table">
          <thead>
            <tr>
              <th>Row</th>
              {quasiColumns.map((c) => (
                <th key={c.name}>{c.name}</th>
              ))}
              <th>{modelLabel} score</th>
              <th>Share of dataset risk</th>
            </tr>
          </thead>
          <tbody>
            {ranked.map(({ index, score, share }) => (
              <tr key={index} title={rowRiskDetail(score, model)}>
                <td>#{index + 1}</td>
                {quasiColumns.map((c) => (
                  <td key={c.name}>{sample?.getCell(index, colIndex[c.name])}</td>
                ))}
                <td>
                  <span
                    className="row-risk-swatch"
                    style={{
                      background: readRiskColorStops().riskColor(
                        Math.min(1, score / ranked[0].score),
                      ),
                    }}
                  />
                  {score.toFixed(1)}
                </td>
                <td>{(share * 100).toFixed(1)}%</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
