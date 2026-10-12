import { lazy, Suspense, useState } from "react";
import { ChevronsRight } from "lucide-react";
import LoadingOverlay from "./LoadingOverlay";
import HistoryTab from "./HistoryTab";
import SynthesisReport from "./SynthesisReport";

const ColumnViewTab = lazy(() => import("./ColumnViewTab"));
const DiversityCheckSection = lazy(() => import("./DiversityCheckSection"));

// History is always last. The two analysis tabs describe whichever dataset is showing, and say so
// in their label when it is not the original.
function overlayTabs(generatedLabel) {
  const suffix = generatedLabel ? ` (${generatedLabel} Data)` : "";
  return [
    { id: "columns", label: `Columns${suffix}` },
    { id: "analysis", label: `Privacy Analysis${suffix}` },
    { id: "history", label: "History" },
  ];
}

/** Slide panel docked to the right edge of the Table view, drawn on top of
 * the grid rather than beside it — collapsed to a narrow always-visible
 * rail by default (the table is the one main view now), expanding on click
 * into a wide panel with its own two tabs: the per-column cards
 * (`ColumnViewTab`) and the dataset-level matrices/row-risk list
 * (`DiversityCheckSection`) — previously the app's second top-level tab,
 * consolidated in here instead.
 *
 * Only the active inner tab is mounted, the same conditional the old
 * top-level tab switch already did — neither tab owns any of its own data
 * fetching (both are fed by hooks App.jsx already runs as soon as the file
 * loads, independent of what's mounted here), so switching tabs, or
 * collapsing and re-expanding, never re-triggers a computation. */
export default function AnalysisOverlay({
  columnProfile,
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
  diversityRaw,
  diversityLoading,
  diversityError,
  quasiColumns,
  allColumns,
  hoverNames,
  onHoverNamesChange,
  sample,
  rowRiskModel,
  onRowRiskModelChange,
  onJointKCellClick,
  methods,
  recipe,
  salt,
  onPickMethod,
  onClearMethod,
  onSaltChange,
  onAnonymize,
  anonymizing,
  columnSubsets,
  historyEntries,
  onUndoHistoryEntry,
  synthesisResult,
  generatedLabel,
  readOnly,
  association,
  openRequest,
}) {
  const tabs = overlayTabs(readOnly ? (generatedLabel ?? "Synthetic") : null);
  const [expanded, setExpanded] = useState(false);
  const [innerTab, setInnerTab] = useState("columns");
  // `openRequest` ({ tab, seq }) lets the Tier picker open a tab from outside: a new `seq` means
  // "open this tab now". Adjusted during render (React's documented pattern for a prop change),
  // not in an effect.
  const [seenRequest, setSeenRequest] = useState(openRequest?.seq ?? 0);
  if (openRequest && openRequest.seq !== seenRequest) {
    setSeenRequest(openRequest.seq);
    setInnerTab(openRequest.tab);
    setExpanded(true);
  }

  const openTab = (id) => {
    setInnerTab(id);
    setExpanded(true);
  };

  if (!expanded) {
    return (
      <div className="analysis-overlay collapsed">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            className="analysis-overlay-rail-btn"
            onClick={() => openTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
    );
  }

  return (
    <div className="analysis-overlay expanded">
      <div className="analysis-overlay-header">
        <div className="analysis-overlay-tabs">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              className={`analysis-overlay-tab-btn${innerTab === tab.id ? " active" : ""}`}
              onClick={() => setInnerTab(tab.id)}
            >
              {tab.label}
            </button>
          ))}
        </div>
        <button
          type="button"
          className="analysis-overlay-collapse-btn"
          onClick={() => setExpanded(false)}
          aria-label="Collapse panel"
          title="Collapse"
        >
          <ChevronsRight size={16} strokeWidth={2} aria-hidden="true" />
        </button>
      </div>
      <div className="analysis-overlay-body">
        <Suspense fallback={<LoadingOverlay label="Loading…" />}>
          {innerTab === "columns" && (
            <ColumnViewTab
              profile={columnProfile}
              selectedColumn={selectedColumn}
              onSelectColumn={onSelectColumn}
              contract={contract}
              onCheckPattern={onCheckPattern}
              patternChecks={patternChecks}
              checkingColumns={checkingColumns}
              anonymizeResult={anonymizeResult}
              riskByName={riskByName}
              riskLoading={riskLoading}
              onDeclareAsString={onDeclareAsString}
              highlightColumnNames={highlightColumnNames}
              onColumnHover={onColumnHover}
              methods={methods}
              recipe={recipe}
              salt={salt}
              onPickMethod={onPickMethod}
              onClearMethod={onClearMethod}
              onSaltChange={onSaltChange}
              onAnonymize={onAnonymize}
              anonymizing={anonymizing}
              onBinClick={onJointKCellClick}
              columnSubsets={columnSubsets}
              readOnly={readOnly}
              association={association}
              hoverNames={hoverNames}
              onHoverNamesChange={onHoverNamesChange}
            />
          )}
          {innerTab === "analysis" && readOnly && synthesisResult && (
            <SynthesisReport result={synthesisResult} />
          )}
          {innerTab === "analysis" && readOnly && (
            <h4 className="synth-matrix-heading">Risk analysis of the generated table</h4>
          )}
          {innerTab === "analysis" && (
            <DiversityCheckSection
              raw={diversityRaw}
              loading={diversityLoading}
              error={diversityError}
              quasiColumns={quasiColumns}
              allColumns={allColumns}
              hoverNames={hoverNames}
              onHoverNamesChange={onHoverNamesChange}
              sample={sample}
              rowRiskModel={rowRiskModel}
              onRowRiskModelChange={onRowRiskModelChange}
              onJointKCellClick={onJointKCellClick}
            />
          )}
          {innerTab === "history" && (
            <HistoryTab entries={historyEntries} onUndo={onUndoHistoryEntry} />
          )}
        </Suspense>
      </div>
    </div>
  );
}
