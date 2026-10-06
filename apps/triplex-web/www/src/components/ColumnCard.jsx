import { MoreVertical, Undo2, X } from "lucide-react";
import HeaderSparkline from "./HeaderSparkline";
import { readRiskColorStops, reciprocalRisk } from "../state/palette";
import { kAnonymityExplanation, distinctCountExplanation } from "../state/riskExplanations";
import { METHOD_META, METHOD_META_FALLBACK } from "./ColumnMenu.const";
import TypeIcon from "./typeIcons";
import InfoTrigger from "./InfoTrigger";

/** One column's detail card in the Column-view tab: distribution sparkline,
 * stats, and (once a pattern is set and the file has been anonymized) a
 * live conformance indicator. Split out from `ColumnViewTab` so each
 * column's rendering/props are self-contained instead of inlined in a
 * `.map()`.
 *
 * `onOpenMenu`, if given, renders the same menu-icon affordance the Table
 * tab's header has next to a column's name — opening the same `ColumnMenu`
 * (anonymization method, salt), not a card-only duplicate of those actions.
 *
 * `appliedMethod` is this column's current recipe entry (if any) — the
 * same value `ColumnMenu`'s method grid would show as selected. "drop"
 * ("Delete from output") gets its own card-level treatment: this is the
 * one state where the card keeps showing even though the column has
 * disappeared from the Table tab's grid (see `useProfiledFile.js`'s
 * `pickMethod`), muted (grayscale, not just faded, since the histogram's
 * own color would otherwise just look like a lighter version of itself
 * rather than "off"), with the menu trigger swapped for a one-click undo.
 * Any other method gets a small removable pill instead, in the same
 * category color `ColumnMenu`'s method-card uses, so a real anonymization
 * choice is visible on the card without reopening the whole menu to check
 * or reverse it -- `onClearMethod(col.name)` (shared with the drop-undo
 * button) un-declares it either way.
 *
 * `onBinClick`, forwarded to `HeaderSparkline`, is the same row-filter
 * setter a Privacy Analysis matrix click already drives (`useRowFilter.js`)
 * -- clicking or dragging across bars filters the Table tab to that
 * bin/value span. `subsetCounts` (this column's own entry from
 * useColumnSubsetDistributions.js), also forwarded, is what turns every
 * card's sparkline into the coordinated-view overlay once any row filter is
 * active, not just the card that set it. */
export default function ColumnCard({
  col,
  risk,
  selected,
  onSelect,
  pattern,
  anonymizeResult,
  onCheckPattern,
  declaredType,
  onDeclareAsString,
  highlighted,
  onHoverChange,
  onOpenMenu,
  readOnly,
  appliedMethod,
  onClearMethod,
  onBinClick,
  subsetCounts,
}) {
  const { value: riskValue, loading: riskLoading } = risk;
  const { declared: declaredPattern, check, checking } = pattern;
  const patternError = check?.errors?.find(
    (e) => e.kind === "pattern_mismatch" && e.name === col.name,
  );
  const isDroppedFromOutput = appliedMethod === "drop";
  const pillMethod = appliedMethod && !isDroppedFromOutput ? appliedMethod : null;
  const pillMeta = pillMethod ? (METHOD_META[pillMethod] ?? METHOD_META_FALLBACK) : null;

  return (
    <div
      className={`column-card${selected ? " selected" : ""}${highlighted ? " highlighted" : ""}${isDroppedFromOutput ? " muted" : ""}`}
      onClick={onSelect}
      onMouseEnter={() => onHoverChange?.(col.name)}
      onMouseLeave={() => onHoverChange?.(null)}
    >
      <div className="column-card-head">
        <span className="column-card-title">
          <h3>{col.name}</h3>
          {pillMethod && (
            <span className={`column-card-method-pill method-card-${pillMeta.category}`}>
              <pillMeta.icon size={10} strokeWidth={2} aria-hidden="true" />
              {pillMeta.label ?? pillMethod}
              <button
                type="button"
                aria-label={`Remove ${pillMeta.label ?? pillMethod} from ${col.name}`}
                title="Undo this anonymization"
                onClick={(e) => {
                  e.stopPropagation();
                  onClearMethod?.(col.name);
                }}
              >
                <X size={9} strokeWidth={2.5} aria-hidden="true" />
              </button>
            </span>
          )}
        </span>
        {isDroppedFromOutput ? (
          <button
            type="button"
            className="column-card-menu-btn"
            aria-label={`Undo delete from output for ${col.name}`}
            title="Deleted from output — undo"
            onClick={(e) => {
              e.stopPropagation();
              onClearMethod?.(col.name);
            }}
          >
            <Undo2 size={16} strokeWidth={2} aria-hidden="true" />
          </button>
        ) : (
          onOpenMenu && (
            <button
              type="button"
              className="column-card-menu-btn"
              aria-label={`${col.name} actions`}
              onClick={(e) => {
                e.stopPropagation();
                const rect = e.currentTarget.getBoundingClientRect();
                onOpenMenu(col.name, rect.left, rect.bottom + 4);
              }}
            >
              <MoreVertical size={16} strokeWidth={2} aria-hidden="true" />
            </button>
          )
        )}
      </div>
      <HeaderSparkline
        distribution={col.distribution}
        columnName={col.name}
        onBinClick={onBinClick}
        subsetCounts={subsetCounts}
      />
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
              {col.semantic_type === "likely_identifier" &&
                !readOnly &&
                (declaredType === "string" ? (
                  <span className="declare-string-done">Declared as string</span>
                ) : (
                  <button
                    type="button"
                    className="declare-string-action"
                    onClick={(e) => {
                      e.stopPropagation();
                      onDeclareAsString(col.name);
                    }}
                  >
                    Declare as string
                  </button>
                ))}
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
              <InfoTrigger
                align="left"
                tooltip={distinctCountExplanation(col.distribution.cardinality)}
              >
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
      {!readOnly && (
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
      )}
    </div>
  );
}
