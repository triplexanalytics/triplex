import { IMPUTATION_OPTIONS } from "./ColumnMenu.const";

// One-line descriptions for the info tooltip next to each method radio.
// `methods` itself comes from `list_anonymize_methods()` (backend stays the
// source of truth for *which* methods exist); this map only supplies the
// human-readable blurb and is missing-safe for a method it doesn't know yet.
// "Read more" is inert until the glossary page (see task) exists to link to.
const METHOD_INFO = {
  token_hmac:
    "Deterministic HMAC token: the same input always produces the same pseudonym under a given salt, so joins across columns/files still work. Not reversible without the salt.",
};

/** Anchored dropdown opened from the small icon next to a column's name in
 * the Table tab's header — the per-column decisions (anonymization now,
 * imputation once it exists) live here instead of a fixed corner panel. */
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
  onClose,
}) {
  return (
    <>
      <div className="menu-backdrop" onClick={onClose} />
      <div className="column-menu" style={{ position: "fixed", left: x, top: y }}>
        <div className="column-menu-header">{columnName}</div>

        <div className="column-menu-section">
          <div className="column-menu-section-title">Anonymization</div>
          <ul>
            {methods.map((method) => (
              <li key={method}>
                <label>
                  <input
                    type="radio"
                    name="algorithm"
                    checked={currentMethod === method}
                    onChange={() => onPickMethod(method)}
                  />{" "}
                  {method}
                </label>
                {METHOD_INFO[method] && (
                  <span className="info-trigger" tabIndex={0}>
                    ⓘ
                    <span className="method-tooltip">
                      {METHOD_INFO[method]}
                      <span className="read-more">Read more…</span>
                    </span>
                  </span>
                )}
              </li>
            ))}
            {methods.length === 0 && <li>No methods available.</li>}
          </ul>
          <div className="field">
            <label>Salt / passphrase (one salt for the whole run)</label>
            <input
              type="password"
              value={currentSalt}
              onChange={(e) => onSaltChange(e.target.value)}
            />
            <p className="field-hint">
              Shared across every column — the same source value must map to the same token
              wherever it recurs for joins to still work; a different salt per column would break
              that.
            </p>
          </div>
          <button
            className="primary"
            onClick={onAnonymize}
            disabled={!currentMethod || !currentSalt}
          >
            Anonymize
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
