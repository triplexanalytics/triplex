import { useState } from "react";
import ColumnCard from "./ColumnCard";
import ColumnMenu from "./ColumnMenu";
import { AssociationMatrix } from "./AssociationMatrix";
import { recipeSettingsText } from "../state/recipeFile";

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
 * on the other end of it live outside this whole overlay.
 *
 * `methods`/`recipe`/`salt`/`onPickMethod`/`onClearMethod`/`onSaltChange`/
 * `onAnonymize`/`anonymizing` are the same anonymization-workflow props
 * `TableTab` already takes — a card's menu button opens the exact same
 * `ColumnMenu` the table header's menu icon does, not a card-only
 * re-implementation of those actions, so `menu` state here mirrors
 * `TableTab.jsx`'s own `menu` state one-for-one. Each card also gets the
 * column's current recipe method (`appliedMethod`), so it can show its own
 * pill/muted-state reflection of that without reopening the menu — see
 * ColumnCard's own doc comment for what each method does with it.
 *
 * `association` (useDatasetAnalysis) is the column-to-column correlation matrix; it always sits
 * under the cards, for whichever dataset is showing.
 *
 * `readOnly` (the synthetic dataset) hides every original-only action: the method menu, the
 * pattern field and "Declare as string".
 *
 * `onBinClick`/`columnSubsets` are useRowFilter.js's setter and
 * useColumnSubsetDistributions.js's per-column result, both from App.jsx —
 * `columnSubsets?.[col.name]` is threaded to each card individually so
 * ColumnCard/HeaderSparkline never need to know the shape of the whole map. */
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
  methods,
  recipe,
  salt,
  onPickMethod,
  onClearMethod,
  onSaltChange,
  onAnonymize,
  anonymizing,
  onBinClick,
  columnSubsets,
  readOnly,
  association,
  hoverNames,
  onHoverNamesChange,
}) {
  const [menu, setMenu] = useState(null); // { name, x, y } | null

  if (!profile) return null;

  return (
    <>
      <div className="column-view-grid">
        {profile.columns.map((col) => (
          <ColumnCard
            key={col.name}
            col={col}
            risk={{
              value: riskByName[col.name],
              loading:
                riskLoading &&
                (col.inferred_type === "integer" || col.distribution?.kind === "categorical"),
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
            readOnly={readOnly}
            onOpenMenu={readOnly ? undefined : (name, x, y) => setMenu({ name, x, y })}
            appliedMethod={recipe.columns.find((c) => c.name === col.name)?.method}
            onClearMethod={onClearMethod}
            onBinClick={onBinClick}
            subsetCounts={columnSubsets?.[col.name]}
          />
        ))}
        {menu && (
          <ColumnMenu
            columnName={menu.name}
            x={menu.x}
            y={menu.y}
            methods={methods}
            currentMethod={recipe.columns.find((c) => c.name === menu.name)?.method}
            currentSettings={recipeSettingsText(recipe.columns.find((c) => c.name === menu.name))}
            currentSalt={salt}
            onPickMethod={(method) => onPickMethod(menu.name, method)}
            onSaltChange={onSaltChange}
            onAnonymize={onAnonymize}
            anonymizing={anonymizing}
            onClose={() => setMenu(null)}
            isLikelyIdentifier={
              profile.columns.find((c) => c.name === menu.name)?.semantic_type ===
              "likely_identifier"
            }
            declaredAsString={
              contract?.columns.find((c) => c.name === menu.name)?.type === "string"
            }
            onDeclareAsString={onDeclareAsString}
          />
        )}
      </div>
      <section className="column-association">
        <h3>Column associations</h3>
        <p className="diversity-check-intro">
          How strongly each pair of columns moves together (see the info icon for the measures).
        </p>
        <AssociationMatrix
          association={association}
          names={undefined}
          title="association"
          hoverNames={hoverNames}
          onHoverNamesChange={onHoverNamesChange}
        />
      </section>
    </>
  );
}
