import { useCallback, useRef, useState } from "react";

/** Native HTML5 drag-and-drop + click-to-browse — no library needed. Hands
 * off the same `File` object the navbar's plain file input already uses, so
 * `App.jsx`'s `handleFile` doesn't care which path a file came in through. */
export default function Dropzone({ onFileSelected }) {
  const inputRef = useRef(null);
  const [dragging, setDragging] = useState(false);

  const handleDrop = useCallback(
    (e) => {
      e.preventDefault();
      setDragging(false);
      const file = e.dataTransfer.files?.[0];
      if (file) onFileSelected(file);
    },
    [onFileSelected],
  );

  return (
    <div
      className={`dropzone${dragging ? " dragging" : ""}`}
      onDragOver={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={handleDrop}
      onClick={() => inputRef.current?.click()}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") inputRef.current?.click();
      }}
    >
      <p>Drag a .csv, .tsv, or .jsonl file here, or click to choose one.</p>
      <p className="hint">
        .parquet isn't supported in the browser (its codecs need a C toolchain
        wasm32-unknown-unknown doesn't have) — the <code>triplex</code> CLI handles it natively.
      </p>
      <input
        ref={inputRef}
        type="file"
        accept=".csv,.tsv,.jsonl"
        hidden
        onChange={(e) => e.target.files[0] && onFileSelected(e.target.files[0])}
      />
    </div>
  );
}
