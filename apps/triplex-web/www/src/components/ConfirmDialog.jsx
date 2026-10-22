import { useEffect, useRef } from "react";

/** A small modal question with a confirm and a cancel button (native <dialog>, like
 * CompareDialog). Not `window.confirm`: notebook front ends that host the Jupyter widget can
 * block browser dialogs, and this one matches the page. Escape and the backdrop cancel. */
export default function ConfirmDialog({ title, confirmLabel, onConfirm, onCancel, children }) {
  const ref = useRef(null);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog || dialog.open) return;
    // jsdom has no showModal; the open attribute is enough to render it there.
    if (typeof dialog.showModal === "function") dialog.showModal();
    else dialog.setAttribute("open", "");
  }, []);
  return (
    <dialog
      ref={ref}
      className="confirm-dialog"
      aria-labelledby="confirm-dialog-title"
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
      onClick={(e) => {
        if (e.target === ref.current) onCancel();
      }}
    >
      <h3 id="confirm-dialog-title">{title}</h3>
      {children}
      <div className="confirm-dialog-actions">
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
        <button type="button" className="primary" onClick={onConfirm} autoFocus>
          {confirmLabel}
        </button>
      </div>
    </dialog>
  );
}
