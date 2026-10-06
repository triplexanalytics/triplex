import { useEffect, useRef } from "react";
import SynthesizeForm from "./SynthesizeForm";

/** Tier 2 (synthetic data) as a modal over the table. Uses the native <dialog> (focus trap, Esc to
 * close, backdrop) rather than a hand-rolled overlay. Rendered only while open by the caller, so
 * `showModal` runs on mount. */
export default function SynthesizeDialog({ synthesis, fileName, onClose, onDone }) {
  const ref = useRef(null);
  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
  }, []);

  const run = async () => {
    if (await synthesis.run()) onDone();
  };

  return (
    <dialog
      ref={ref}
      className="synth-dialog"
      onClose={onClose}
      aria-label="Generate synthetic data"
    >
      <div className="synth-dialog-head">
        <h3>Generate synthetic data</h3>
        <button type="button" onClick={() => ref.current?.close()} aria-label="Close">
          Close
        </button>
      </div>
      <div className="synth-dialog-body">
        <SynthesizeForm synthesis={synthesis} hasFile fileName={fileName} onRun={run} />
      </div>
    </dialog>
  );
}
