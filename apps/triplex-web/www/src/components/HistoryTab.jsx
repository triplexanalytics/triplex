import { Undo2 } from "lucide-react";

/** Read-only log of operations performed this session, each with an Undo
 * button -- see useHistory.js for what undo actually restores (and its
 * ponytail-marked limitation). Newest entry first, matching how the log is
 * built. */
export default function HistoryTab({ entries, onUndo }) {
  if (entries.length === 0) {
    return <p className="row-risk-empty">No operations yet.</p>;
  }

  return (
    <table className="history-table">
      <thead>
        <tr>
          <th className="history-col-time">Time Stamp</th>
          <th className="history-col-op">Operation</th>
          <th className="history-col-count">Column/Row count</th>
          <th className="history-col-column">Column</th>
          <th className="history-col-undo" />
        </tr>
      </thead>
      <tbody>
        {entries.map((e) => (
          <tr key={e.id}>
            <td className="history-col-time">{e.timestamp.toLocaleTimeString()}</td>
            <td className="history-col-op">{e.op}</td>
            <td className="history-col-count">{e.count}</td>
            <td className="history-col-column">{e.column ?? "–"}</td>
            <td className="history-col-undo">
              {e.undo && (
                <button
                  type="button"
                  className="history-undo-btn"
                  aria-label={`Undo ${e.op}`}
                  title="Undo"
                  onClick={() => onUndo(e.id)}
                >
                  <Undo2 size={14} strokeWidth={2} aria-hidden="true" />
                </button>
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
