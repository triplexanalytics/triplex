import ColumnCard from "./ColumnCard";

/** Grid of per-column detail cards — one `ColumnCard` per profiled column,
 * fed the same profile that drives the Table tab's header sparklines.
 * `riskByName`/`riskLoading` come from `useColumnRisk` in App.jsx, shared
 * with the Table tab's header bar rather than computed twice.
 * `DiversityCheckSection` renders as App.jsx's sibling now, not nested
 * here — same DOM order either way, but its data (`useDiversityMatrix`)
 * needed to be owned alongside `useColumnRisk` so both start computing as
 * soon as the file loads instead of the matrix lagging a beat behind the
 * k-anonymity bars every time this tab is (re-)opened. */
export default function ColumnViewTab({
  profile,
  selectedColumn,
  onSelectColumn,
  contract,
  onCheckPattern,
  patternChecks,
  checkingColumns,
  anonymizeResult,
  riskByName,
  riskLoading,
}) {
  if (!profile) return null;

  return (
    <div className="column-view-grid">
      {profile.columns.map((col) => (
        <ColumnCard
          key={col.name}
          col={col}
          risk={{
            value: riskByName[col.name],
            loading: riskLoading && col.inferred_type === "integer",
          }}
          selected={col.name === selectedColumn}
          onSelect={() => onSelectColumn(col.name)}
          pattern={{
            declared: contract?.columns.find((c) => c.name === col.name)?.pattern,
            check: patternChecks[col.name],
            checking: checkingColumns.has(col.name),
          }}
          anonymizeResult={anonymizeResult}
          onCheckPattern={onCheckPattern}
        />
      ))}
    </div>
  );
}
