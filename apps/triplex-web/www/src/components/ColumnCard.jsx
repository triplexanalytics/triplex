import HeaderSparkline from "./HeaderSparkline";
import { readRiskColorStops, reciprocalRisk } from "../state/palette";
import { kAnonymityExplanation, distinctCountExplanation } from "../state/riskExplanations";
import TypeIcon from "./typeIcons";
import InfoTrigger from "./InfoTrigger";

/** One column's detail card in the Column-view tab: distribution sparkline,
 * stats, and (once a pattern is set and the file has been anonymized) a
 * live conformance indicator. Split out from `ColumnViewTab` so each
 * column's rendering/props are self-contained instead of inlined in a
 * `.map()`. */
export default function ColumnCard({
  col,
  risk,
  selected,
  onSelect,
  pattern,
  anonymizeResult,
  onCheckPattern,
  onHoverChange,
}) {
  const { value: riskValue, loading: riskLoading } = risk;
  const { declared: declaredPattern, check, checking } = pattern;
  const patternError = check?.errors?.find(
    (e) => e.kind === "pattern_mismatch" && e.name === col.name,
  );

  return (
    <div
      className={`column-card${selected ? " selected" : ""}`}
      onClick={onSelect}
      onMouseEnter={() => onHoverChange?.(col.name)}
      onMouseLeave={() => onHoverChange?.(null)}
    >
      <h3>{col.name}</h3>
      <HeaderSparkline distribution={col.distribution} />
      <dl>
        <dt>Type</dt>
        <dd className="type-with-icon">
          <TypeIcon type={col.inferred_type} />
          {col.inferred_type}
        </dd>
        {col.semantic_type && (
          <>
            <dt>Detected</dt>
            <dd className="type-with-icon">
              <TypeIcon type={col.semantic_type} />
              {col.semantic_type}
            </dd>
          </>
        )}
        <dt>Missing</dt>
        <dd>{col.null_percentage.toFixed(1)}%</dd>
        <dt>Mismatched</dt>
        <dd>{(100 - col.type_consistency_percentage).toFixed(1)}%</dd>
        {col.distribution?.kind === "numeric" && (
          <>
            <dt>Median</dt>
            <dd>{col.distribution.median?.toFixed(2) ?? "N/A"}</dd>
            <dt>Std dev</dt>
            <dd>{col.distribution.std_dev?.toFixed(2) ?? "N/A"}</dd>
          </>
        )}
        {col.distribution?.kind === "categorical" && (
          <>
            <dt>
              <InfoTrigger align="left" tooltip={distinctCountExplanation(col.distribution.cardinality)}>
                Distinct
              </InfoTrigger>
            </dt>
            <dd>{col.distribution.cardinality}</dd>
          </>
        )}
        {riskLoading && (
          <>
            <dt>Privacy risk</dt>
            <dd>
              <span className="skeleton-line" />
            </dd>
          </>
        )}
        {!riskLoading && riskValue?.model === "k_anonymity" && (
          <>
            <dt>Privacy risk</dt>
            <dd>
              <InfoTrigger wide tooltip={kAnonymityExplanation(riskValue.value)}>
                <div className="risk-bar-track">
                  <div
                    className="risk-bar-fill"
                    style={{
                      width: `${reciprocalRisk(riskValue.value) * 100}%`,
                      background: readRiskColorStops().riskColor(reciprocalRisk(riskValue.value)),
                    }}
                  />
                </div>
              </InfoTrigger>
              <span className="risk-bar-label">k-anonymity {riskValue.value}</span>
            </dd>
          </>
        )}
      </dl>
      <div className="field" onClick={(e) => e.stopPropagation()}>
        <label>Pattern (regex)</label>
        <input
          type="text"
          defaultValue={declaredPattern || ""}
          onBlur={(e) => onCheckPattern(col.name, e.target.value)}
        />
        {anonymizeResult && checking && (
          <div>
            <span className="skeleton-line" />
          </div>
        )}
        {anonymizeResult && !checking && check && (
          <div className={patternError ? "conformance-bad" : "conformance-ok"}>
            {patternError
              ? `${patternError.non_conforming_count}/${patternError.checked_count} values no longer match`
              : "Still conforms after anonymize"}
          </div>
        )}
      </div>
    </div>
  );
}
