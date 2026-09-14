import { useMemo } from "react";
import InfoTrigger from "./InfoTrigger";
import { readRiskColorStops } from "../state/palette";
import { ROW_RISK_INFO, rowRiskDetail } from "../state/riskExplanations";

const TOP_N = 10;

/** Dataset Anonymity Analysis tab: the one row-level (not column or
 * column-pair) risk view in the app — every matrix above answers "how
 * safe is this column/pair," this answers "which specific rows are most
 * identifiable" (SUDA/DIS-SUDA). Ranks the riskiest rows
 * by SUDA score, capped to the top `TOP_N` so a large file doesn't dump a
 * score for every single row; rows scoring 0 (never part of any minimal
 * unique combination) are left out entirely rather than padding the list
 * down to `TOP_N` with rows that carry no signal.
 *
 * Shows each row by its 1-based position (still useful to cross-reference
 * the Table tab) plus its actual value on every quasi-identifier column —
 * not the candidate sensitive column's value, since SUDA scores are
 * computed from the quasi-identifier columns alone, so those are what
 * explain *why* the row stood out. */
export default function RowRiskSection({ raw, loading, quasiColumns, sample }) {
  const ranked = useMemo(() => {
    if (!raw?.rowRisk) return null;
    return raw.rowRisk
      .map(([suda, disSuda], index) => ({ index, suda, disSuda }))
      .filter((row) => row.suda > 0)
      .sort((a, b) => b.suda - a.suda)
      .slice(0, TOP_N);
  }, [raw]);

  const colIndex = useMemo(() => {
    if (!sample) return {};
    const map = {};
    sample.headers.forEach((name, i) => (map[name] = i));
    return map;
  }, [sample]);

  if (quasiColumns.length === 0) return null;

  return (
    <div className="row-risk-section">
      <h3>
        Row risk (SUDA) <InfoTrigger tooltip={ROW_RISK_INFO} align="left" />
      </h3>
      {loading || !raw ? (
        <p className="row-risk-empty">Scoring rows…</p>
      ) : !raw.rowRisk ? (
        <p className="row-risk-empty">
          Not shown for more than 10 quasi-identifier columns at once (cost grows exponentially
          with column count) — this file has {quasiColumns.length}.
        </p>
      ) : ranked.length === 0 ? (
        <p className="row-risk-empty">
          No row is uniquely identified by any combination of the {quasiColumns.length}{" "}
          quasi-identifier column{quasiColumns.length === 1 ? "" : "s"} checked.
        </p>
      ) : (
        <table className="row-risk-table">
          <thead>
            <tr>
              <th>Row</th>
              {quasiColumns.map((c) => (
                <th key={c.name}>{c.name}</th>
              ))}
              <th>SUDA score</th>
              <th>Share of dataset risk</th>
            </tr>
          </thead>
          <tbody>
            {ranked.map(({ index, suda, disSuda }) => (
              <tr key={index} title={rowRiskDetail(suda)}>
                <td>#{index + 1}</td>
                {quasiColumns.map((c) => (
                  <td key={c.name}>{sample?.getCell(index, colIndex[c.name])}</td>
                ))}
                <td>
                  <span
                    className="row-risk-swatch"
                    style={{ background: readRiskColorStops().riskColor(Math.min(1, suda / ranked[0].suda)) }}
                  />
                  {suda.toFixed(1)}
                </td>
                <td>{(disSuda * 100).toFixed(1)}%</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
