import ColumnCard from "./ColumnCard";

/** Grid of per-column detail cards — one `ColumnCard` per profiled column,
 * fed the same profile that drives the Table tab's header sparklines.
 * `riskByName`/`riskLoading` come from `useColumnRisk` in App.jsx, shared
 * with the Table tab's header bar rather than computed twice.
 * `DiversityCheckSection` is this component's sibling under
 * `AnalysisOverlay` now, its own inner tab rather than stacked below this
 * one — its data (`useDiversityMatrix`) is still owned alongside
 * `useColumnRisk` in App.jsx, so both start computing as soon as the file
 * loads instead of lagging a beat behind whenever the overlay is opened.
 * `highlightColumnNames`/`onColumnHover` are the table/matrix<->card
 * cross-highlight, lifted to App.jsx since the table header and matrices
 * on the other end of it live outside this whole overlay. */
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
  onDeclareAsString,
  highlightColumnNames,
  onColumnHover,
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
            loading: riskLoading && (col.inferred_type === "integer" || col.distribution?.kind === "categorical"),
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
          declaredType={contract?.columns.find((c) => c.name === col.name)?.type}
          onDeclareAsString={onDeclareAsString}
          highlighted={highlightColumnNames?.includes(col.name)}
          onHoverChange={onColumnHover}
        />
      ))}
    </div>
  );
}
