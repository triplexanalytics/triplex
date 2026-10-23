import { useEffect, useRef } from "react";
import CompareView from "./CompareView";

/** The Compare view as a large modal over the table (native <dialog>, like SynthesizeDialog).
 * Rendered only while open by the caller, so `showModal` runs on mount. `choices` lists what can
 * be set beside the original (`{ key, label }`); with more than one, the header switches between
 * them. `stale` says the right side is from a run whose recipe has changed since. */
export default function CompareDialog({
  left,
  right,
  quasi,
  stale = false,
  choices = [],
  active,
  onChoose,
  hoverNames,
  onHoverNamesChange,
  onColumnHover,
  onClose,
}) {
  const ref = useRef(null);
  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
  }, []);
  return (
    <dialog ref={ref} className="compare-dialog" onClose={onClose} aria-label="Compare datasets">
      <div className="compare-dialog-head">
        <h3>
          Compare: {left.label} and {right.label}
        </h3>
        <button type="button" onClick={() => ref.current?.close()} aria-label="Close">
          Close
        </button>
      </div>
      {(choices.length > 1 || stale) && (
        <div className="compare-dialog-bar">
          {choices.length > 1 && (
            <div className="compare-choices" role="group" aria-label="Compare the original with">
              <span>Compare the original with:</span>
              {choices.map((c) => (
                <button
                  key={c.key}
                  type="button"
                  aria-pressed={c.key === active}
                  className={c.key === active ? "active" : undefined}
                  onClick={() => onChoose(c.key)}
                >
                  {c.label}
                </button>
              ))}
            </div>
          )}
          {stale && (
            <p className="compare-stale" role="note">
              From the last run. The recipe or the removed rows have changed since; run Anonymize
              again to compare the current recipe.
            </p>
          )}
        </div>
      )}
      <CompareView
        left={left}
        right={right}
        quasi={quasi}
        hoverNames={hoverNames}
        onHoverNamesChange={onHoverNamesChange}
        onColumnHover={onColumnHover}
      />
    </dialog>
  );
}
