import { useCallback, useRef, useState } from "react";

/** Append-only log of user-visible operations, newest first -- shown in the
 * History tab. Each entry carries the `undo` closure App.jsx captured at
 * the moment it happened (a snapshot of whatever state that action
 * touched, restored via the same hooks' raw setters) -- clicking Undo runs
 * it and drops the entry, rather than this hook trying to model a general
 * undo/redo stack itself.
 *
 * ponytail: undoing an entry doesn't account for later entries that
 * touched the same state (e.g. picking a second method for a column after
 * undoing the first just reapplies the snapshot on top) -- add proper
 * dependency tracking if that turns out to matter in practice. */
export function useHistory() {
  const [entries, setEntries] = useState([]);
  const nextId = useRef(0);

  const log = useCallback((op, count, column, undo) => {
    const id = nextId.current++;
    setEntries((prev) => [
      { id, timestamp: new Date(), op, count, column: column || null, undo: undo || null },
      ...prev,
    ]);
  }, []);

  const undo = useCallback(
    (id) => {
      entries.find((e) => e.id === id)?.undo?.();
      setEntries((prev) => prev.filter((e) => e.id !== id));
    },
    [entries],
  );

  const clear = useCallback(() => setEntries([]), []);

  return { entries, log, undo, clear };
}
