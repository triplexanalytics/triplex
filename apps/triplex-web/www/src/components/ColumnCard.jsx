import HeaderSparkline from "./HeaderSparkline";

/** One column's detail card in the Column-view tab: distribution sparkline,
 * stats, and (once a pattern is set and the file has been anonymized) a
 * live conformance indicator. Split out from `ColumnViewTab` so each
 * column's rendering/props are self-contained instead of inlined in a
 * `.map()`. */
export default function ColumnCard({
  col,
  selected,
  onSelect,
  declaredPattern,
  check,
  anonymizeResult,
  onCheckPattern,
}) {
  const patternError = check?.errors?.find(
    (e) => e.kind === "pattern_mismatch" && e.name === col.name,
  );

  return (
    <div className={`column-card${selected ? " selected" : ""}`} onClick={onSelect}>
      <h3>{col.name}</h3>
      <HeaderSparkline distribution={col.distribution} />
      <dl>
        <dt>Type</dt>
        <dd>{col.inferred_type}</dd>
        <dt>Missing</dt>
        <dd>{col.null_percentage.toFixed(1)}%</dd>
        <dt>Mismatched</dt>
        <dd>{(100 - col.type_consistency_percentage).toFixed(1)}%</dd>
        {col.distribution?.kind === "numeric" && (
          <>
            <dt>Median</dt>
            <dd>{col.distribution.median?.toFixed(2) ?? "—"}</dd>
            <dt>Std dev</dt>
            <dd>{col.distribution.std_dev?.toFixed(2) ?? "—"}</dd>
          </>
        )}
        {col.distribution?.kind === "categorical" && (
          <>
            <dt>Distinct</dt>
            <dd>{col.distribution.cardinality}</dd>
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
        {anonymizeResult && check && (
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
