import ColumnCard from "./ColumnCard";

/** Grid of per-column detail cards — one `ColumnCard` per profiled column,
 * fed the same profile that drives the Table tab's header sparklines. */
export default function ColumnViewTab({
  profile,
  selectedColumn,
  onSelectColumn,
  contract,
  onCheckPattern,
  patternChecks,
  anonymizeResult,
}) {
  if (!profile) return null;

  return (
    <div className="column-view-grid">
      {profile.columns.map((col) => (
        <ColumnCard
          key={col.name}
          col={col}
          selected={col.name === selectedColumn}
          onSelect={() => onSelectColumn(col.name)}
          declaredPattern={contract?.columns.find((c) => c.name === col.name)?.pattern}
          check={patternChecks[col.name]}
          anonymizeResult={anonymizeResult}
          onCheckPattern={onCheckPattern}
        />
      ))}
    </div>
  );
}
