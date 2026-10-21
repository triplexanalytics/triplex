import { useLayoutEffect, useRef, useState } from "react";
import {
  IMPUTATION_OPTIONS,
  METHOD_INFO,
  METHOD_META,
  METHOD_META_FALLBACK,
} from "./ColumnMenu.const";
import InfoTrigger from "./InfoTrigger";
import ReadMore from "./ReadMore";
import { METHOD_GLOSSARY } from "../state/glossary";

// Space kept between the menu and the window edge.
const MENU_MARGIN = 8;

// "Read more" opens the method's glossary page on the docs site (METHOD_GLOSSARY, ReadMore.jsx).

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
  const menuRef = useRef(null);
  const [pos, setPos] = useState({ x, y });
  // Opened at the pointer, the menu can run past the window's right or bottom edge (a column
  // near the right of the side panel, a header near the bottom). Once it has a size, it moves
  // back inside, keeping a small margin.
  useLayoutEffect(() => {
    const el = menuRef.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    setPos({
      x: Math.max(MENU_MARGIN, Math.min(x, window.innerWidth - width - MENU_MARGIN)),
      y: Math.max(MENU_MARGIN, Math.min(y, window.innerHeight - height - MENU_MARGIN)),
    });
  }, [x, y]);
  return (
    <>
      <div className="menu-backdrop" onClick={onClose} />
      <div
        ref={menuRef}
        className="column-menu"
        style={{ position: "fixed", left: pos.x, top: pos.y }}
      >
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
                            {METHOD_GLOSSARY[method] && <ReadMore term={METHOD_GLOSSARY[method]} />}
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
              Shared across every column: the same source value must map to the same token wherever
              it recurs for joins to still work; a different salt per column would break that.
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
