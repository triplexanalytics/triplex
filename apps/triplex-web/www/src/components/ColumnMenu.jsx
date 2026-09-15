import { IMPUTATION_OPTIONS, METHOD_INFO, METHOD_META, METHOD_META_FALLBACK } from "./ColumnMenu.const";
import InfoTrigger from "./InfoTrigger";

// "Read more" is inert until the glossary page (see task) exists to link to.

/** Anchored dropdown opened from the small icon next to a column's name in
 * the Table tab's header — the per-column decisions (anonymization now,
 * imputation once it exists) live here instead of a fixed corner panel.
 * Picking "drop" (the "Delete from output" method card) is this menu's one
 * destructive-feeling action; it has no confirm() dialog like a plain
 * delete button might, because it isn't a dead end -- the card it hides
 * behind gets a one-click undo instead (see ColumnCard). */
export default function ColumnMenu({
  columnName,
  x,
  y,
  methods,
  currentMethod,
  currentSalt,
  onPickMethod,
  onSaltChange,
  onAnonymize,
  anonymizing,
  onClose,
  isLikelyIdentifier,
  declaredAsString,
  onDeclareAsString,
}) {
  return (
    <>
      <div className="menu-backdrop" onClick={onClose} />
      <div className="column-menu" style={{ position: "fixed", left: x, top: y }}>
        <div className="column-menu-header">
          {columnName}
          {isLikelyIdentifier &&
            (declaredAsString ? (
              <span className="declare-string-done">Declared as string</span>
            ) : (
              <button
                type="button"
                className="declare-string-action"
                onClick={() => onDeclareAsString(columnName)}
              >
                Declare as string
              </button>
            ))}
        </div>

        <div className="column-menu-section">
          <div className="column-menu-section-title">Anonymization</div>
          <div className="method-grid" role="radiogroup" aria-label="Anonymization method">
            {methods.map((method) => {
              const { icon: Icon, category, label } = METHOD_META[method] ?? METHOD_META_FALLBACK;
              const selected = currentMethod === method;
              return (
                <button
                  key={method}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  className={`method-card method-card-${category}${selected ? " selected" : ""}`}
                  onClick={() => onPickMethod(method)}
                >
                  <Icon size={18} strokeWidth={2} aria-hidden="true" />
                  <span className="method-card-name">{label ?? method}</span>
                  {METHOD_INFO[method] && (
                    // stopPropagation: the info glyph sits inside the card's own
                    // click target, and hovering/focusing it must not select the method.
                    <span className="method-card-info" onClick={(e) => e.stopPropagation()}>
                      <InfoTrigger
                        tooltip={
                          <>
                            {METHOD_INFO[method]}
                            <span className="info-trigger-readmore">Read more…</span>
                          </>
                        }
                      />
                    </span>
                  )}
                </button>
              );
            })}
            {methods.length === 0 && <div className="method-grid-empty">No methods available.</div>}
          </div>
          <div className="field">
            <label>Salt / passphrase (one salt for the whole run)</label>
            <input
              type="password"
              value={currentSalt}
              onChange={(e) => onSaltChange(e.target.value)}
            />
            <p className="field-hint">
              Shared across every column: the same source value must map to the same token
              wherever it recurs for joins to still work; a different salt per column would break
              that.
            </p>
          </div>
          <button
            className="primary"
            onClick={onAnonymize}
            disabled={!currentMethod || !currentSalt || anonymizing}
          >
            {anonymizing ? "Anonymizing…" : "Anonymize"}
          </button>
        </div>

        <div className="column-menu-section">
          <div className="column-menu-section-title">
            Imputation <span className="coming-soon">coming soon</span>
          </div>
          <select disabled defaultValue="">
            <option value="" disabled>
              Choose a strategy…
            </option>
            {IMPUTATION_OPTIONS.map((opt) => (
              <option key={opt} value={opt}>
                {opt}
              </option>
            ))}
          </select>
        </div>
      </div>
    </>
  );
}
