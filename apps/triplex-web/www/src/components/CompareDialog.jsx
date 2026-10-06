import { useEffect, useRef } from "react";
import CompareView from "./CompareView";

/** The Compare view as a large modal over the table (native <dialog>, like SynthesizeDialog).
 * Rendered only while open by the caller, so `showModal` runs on mount. */
export default function CompareDialog({
  left,
  right,
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
      <CompareView
        left={left}
        right={right}
        hoverNames={hoverNames}
        onHoverNamesChange={onHoverNamesChange}
        onColumnHover={onColumnHover}
      />
    </dialog>
  );
}
