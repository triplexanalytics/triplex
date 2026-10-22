import { useRef, useState } from "react";

/** Accepts a file dropped anywhere on the page, including over the loaded table, and hands it
 * to `onFile`; without this the browser would open a file dropped there in place of the app.
 * A drop the start screen's Dropzone already took is left alone. `hint` is shown over the page
 * while a file is dragged over it, when given. */
export default function FileDrop({ onFile, hint = null, className, children, ...rest }) {
  // dragenter/dragleave fire for every child crossed, so count them to know when the drag left.
  const depth = useRef(0);
  const [dragging, setDragging] = useState(false);
  const carriesFiles = (e) => [...(e.dataTransfer?.types ?? [])].includes("Files");
  return (
    <div
      className={className}
      {...rest}
      onDragEnter={(e) => {
        if (!carriesFiles(e)) return;
        depth.current += 1;
        setDragging(true);
      }}
      onDragLeave={(e) => {
        if (!carriesFiles(e)) return;
        depth.current = Math.max(0, depth.current - 1);
        if (depth.current === 0) setDragging(false);
      }}
      onDragOver={(e) => {
        if (carriesFiles(e)) e.preventDefault();
      }}
      onDrop={(e) => {
        depth.current = 0;
        setDragging(false);
        if (e.defaultPrevented) return;
        e.preventDefault();
        const file = e.dataTransfer?.files?.[0];
        if (file) onFile(file);
      }}
    >
      {children}
      {dragging && hint && (
        <div className="file-drop-hint" aria-hidden="true">
          <p>{hint}</p>
        </div>
      )}
    </div>
  );
}
