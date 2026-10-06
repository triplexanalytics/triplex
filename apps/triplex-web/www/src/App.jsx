import { lazy, Suspense, useCallback, useMemo, useState } from "react";
import { useWasm } from "./hooks/useWasm";
import { useProfiledFile } from "./hooks/useProfiledFile";
import { useAnonymize } from "./hooks/useAnonymize";
import { useDatasetAnalysis } from "./hooks/useDatasetAnalysis";
import { useHistory } from "./hooks/useHistory";
import { useSynthesize } from "./hooks/useSynthesize";
import { withColumnType } from "./state/yamlBuilder";
import Navbar from "./components/Navbar";
import DatasetTabs from "./components/DatasetTabs";
import SynthesizeDialog from "./components/SynthesizeDialog";
import Dropzone from "./components/Dropzone";
import LoadingOverlay from "./components/LoadingOverlay";

// Code-split, not eagerly bundled with the app shell: TableTab alone pulls
// in @glideapps/glide-data-grid, and AnalysisOverlay's own two inner tabs
// (ColumnViewTab, DiversityCheckSection, lazy-loaded inside it) pull in the
// canvas risk-matrix machinery — none of it is needed until a file is
// actually loaded, and the table still benefits from loading in its own
// chunk in parallel with the rest of the shell instead of being parsed as
// part of one large bundle up front.
const TableTab = lazy(() => import("./components/TableTab"));
const AnalysisOverlay = lazy(() => import("./components/AnalysisOverlay"));
const CompareDialog = lazy(() => import("./components/CompareDialog"));

// What the read-only synthetic view passes where the original view passes its anonymization state:
// the synthetic table has no recipe, contract, pattern checks or removed rows of its own.
const NO_ROWS = new Set();
const NO_RECIPE = { columns: [] };
const NO_PATTERN_CHECKS = {};
const TIER_OF_DATASET = { original: 1, synthetic: 2 };

/** Composition root only: wires the three hooks together and renders. Every
 * actual behavior (wasm lifecycle, file/contract/recipe state, the
 * anonymize/pattern-check workflow) lives in its own hook under `hooks/` —
 * keeping that logic out of this file is deliberate, not an oversight. */
export default function App() {
  const { wasm, methods } = useWasm();
  const file = useProfiledFile(wasm);
  const anonymize = useAnonymize({
    wasm,
    fileBytes: file.fileBytes,
    extension: file.extension,
    recipe: file.recipe,
    salt: file.salt,
    contract: file.contract,
    setContract: file.setContract,
    removedRowIndices: file.removedRowIndices,
  });
  const original = useDatasetAnalysis(wasm, file.sample, file.profile, file.removedRowIndices);
  // Idle until a synthesis run exists (all four hooks accept a null sample).
  const history = useHistory();
  const synthesis = useSynthesize({
    fileBytes: file.fileBytes,
    extension: file.extension,
    headers: file.sample?.headers,
    totalRows: file.sample?.rowCount ?? 0,
    removedRowIndices: file.removedRowIndices,
    removedColumns: file.removedColumns,
  });
  const syntheticResult = synthesis.result;
  const synthetic = useDatasetAnalysis(
    wasm,
    syntheticResult?.sample,
    syntheticResult?.profile,
    NO_ROWS,
  );
  // Which dataset the Table and the panel show. Synthetic (Tier 2) is read-only: no recipe,
  // contract or row removal, and its analysis tabs describe the synthetic table.
  const [activeDataset, setActiveDataset] = useState("original");
  const showingSynthetic = activeDataset === "synthetic" && !!syntheticResult;
  const shownKey = showingSynthetic ? "synthetic" : "original";
  const [compareOpen, setCompareOpen] = useState(false);
  const view = showingSynthetic ? synthetic : original;
  const shown = showingSynthetic ? syntheticResult : file;
  // The Compare view's two sides: the original (minus columns the user removed) and the synthetic
  // table. A later AI-generated or uploaded dataset becomes another right-hand side here.
  const compareLeft = useMemo(
    () =>
      file.sample && file.profile
        ? {
            label: "Original",
            kind: "original",
            sample: file.sample,
            profile: {
              ...file.profile,
              columns: file.profile.columns.filter((c) => !file.removedColumns.has(c.name)),
            },
            analysis: original,
            excluded: file.removedRowIndices,
          }
        : null,
    [file.sample, file.profile, file.removedColumns, file.removedRowIndices, original],
  );
  const compareRight = useMemo(
    () =>
      syntheticResult
        ? {
            label: "Synthetic",
            kind: "synthetic",
            sample: syntheticResult.sample,
            profile: syntheticResult.profile,
            analysis: synthetic,
            excluded: NO_ROWS,
            droppedReasons: Object.fromEntries(
              (syntheticResult.report.columns_dropped ?? []).map((d) => [d.column, d.reason]),
            ),
          }
        : null,
    [syntheticResult, synthetic],
  );
  const [synthDialogOpen, setSynthDialogOpen] = useState(false);
  const [tableWidth, setTableWidth] = useState(null);
  // `seq` makes a repeated request re-open a collapsed panel on the given tab.
  const [overlayRequest, setOverlayRequest] = useState({ tab: "columns", seq: 0 });
  const [selectedColumn, setSelectedColumn] = useState(null);
  // { rowName, colName } | null — the one shared hover state behind every
  // cross-highlight in the app: RiskMatrix cells cross-highlight each other
  // across every matrix in the Analysis inner tab (their own onHoverChange
  // writes straight here), a table header hover sets both names to that
  // column (handleColumnHover below) to highlight the matching ColumnCard
  // in the Columns inner tab, and a ColumnCard hover sets it right back to
  // highlight the matching table header. One piece of state instead of a
  // separate "which column" tracker, since a table/card hover is really
  // just the rowName===colName case of what a matrix cell hover already
  // produces — highlightColumnNames below reads either shape the same way.
  const [hoverNames, setHoverNames] = useState(null);
  // Table header <-> ColumnCard and table header <-> RiskMatrix are both
  // "highlight whichever table headers correspond to hoverNames," so
  // TableTab and ColumnCard both just check membership in this list rather
  // than each re-deriving it from hoverNames themselves. A matrix cell's
  // rowName/colName can be two different columns (e.g. hovering the
  // id/price cell in a pairwise matrix) or the same one (a table/card
  // hover, or a matrix's own row/column label) — deduped either way. The
  // table header direction uses a DOM overlay computed from
  // DataEditorRef.getBounds() (see TableTab.jsx), not a Glide header
  // canvas repaint, which it won't do for a plain prop change with no
  // accompanying grid interaction (see decision notes in
  // useTableHeaderCanvas.js's git history for the repaint approaches that
  // didn't work: an imperative damage-API repaint that corrupted the
  // header, and a gridKey remount that reset scroll position).
  const highlightColumnNames = hoverNames
    ? [...new Set([hoverNames.rowName, hoverNames.colName].filter(Boolean))]
    : [];
  const selectDataset = (id) => {
    setActiveDataset(id);
    setSelectedColumn(null);
    setHoverNames(null);
  };
  // Tier 1 shows the original; Tier 2 opens the synthesis dialog (also to re-run it).
  const handleSelectTier = (tier) => {
    if (tier === 1) selectDataset("original");
    if (tier === 2) setSynthDialogOpen(true);
  };
  const handleSynthesisDone = () => {
    setSynthDialogOpen(false);
    selectDataset("synthetic");
    setOverlayRequest((r) => ({ tab: "analysis", seq: r.seq + 1 }));
    history.log(
      "Generate synthetic data",
      synthesis.mode === "dp" ? "DP" : "standard",
      null,
      () => {
        synthesis.clearResult();
        selectDataset("original");
      },
    );
  };
  const handleColumnHover = useCallback((name) => {
    setHoverNames(name ? { rowName: name, colName: name } : null);
  }, []);
  const declareAsString = (name) => {
    const prevContract = file.contract;
    file.setContract((c) => c && withColumnType(c, name, "string"));
    history.log("Declare as string", "1 col", name, () => file.setContract(prevContract));
  };
  const handlePickMethod = (columnName, method) => {
    const prevRecipe = file.recipe;
    const prevRemovedColumns = file.removedColumns;
    file.pickMethod(columnName, method);
    history.log(`Anonymize: ${method}`, "1 col", columnName, () => {
      file.setRecipe(prevRecipe);
      file.setRemovedColumns(prevRemovedColumns);
    });
  };
  const handleClearMethod = (columnName) => {
    const prevRecipe = file.recipe;
    const prevRemovedColumns = file.removedColumns;
    file.clearMethod(columnName);
    history.log("Clear method", "1 col", columnName, () => {
      file.setRecipe(prevRecipe);
      file.setRemovedColumns(prevRemovedColumns);
    });
  };
  const handleRestoreRemovedRows = () => {
    const prevRemoved = file.removedRowIndices;
    const n = prevRemoved.size;
    file.restoreRemovedRows();
    history.log("Restore removed rows", `${n} rows`, null, () =>
      file.setRemovedRowIndices(prevRemoved),
    );
  };
  // TableTab's filter banner offers two directions once a filter is active
  // -- both clear the filter afterward, since either way there's nothing
  // left to narrow further (the visible set changed underneath it: shrunk
  // to nothing for the "remove visible" case, grown to everything for
  // "remove filtered", so a stale "Filtered to X of Y" banner sitting on
  // top of the new removed-rows one would just be confusing either way).
  //
  // "Remove visible rows": the rows the filter currently matches (what's
  // shown) are removed; everything else stays.
  const handleRemoveVisibleRows = () => {
    if (!original.rowFilter.filteredRowIndices) return;
    const prevRemoved = file.removedRowIndices;
    const n = original.rowFilter.filteredRowIndices.size;
    file.removeRows(original.rowFilter.filteredRowIndices);
    original.rowFilter.setRowFilter(null);
    history.log("Remove visible rows", `${n} rows`, null, () =>
      file.setRemovedRowIndices(prevRemoved),
    );
  };
  // "Remove filtered rows": the complement -- rows the filter excluded
  // (hidden, not currently shown) are removed, i.e. "keep only what's
  // visible." Computed against rows not already removed, not the full
  // original row count, so a second removal on top of an earlier one only
  // ever shrinks the working set further.
  const handleRemoveFilteredOutRows = () => {
    if (!original.rowFilter.filteredRowIndices || !file.sample) return;
    const hidden = [];
    for (let i = 0; i < file.sample.rowCount; i++) {
      if (!original.rowFilter.filteredRowIndices.has(i) && !file.removedRowIndices.has(i))
        hidden.push(i);
    }
    const prevRemoved = file.removedRowIndices;
    file.removeRows(hidden);
    original.rowFilter.setRowFilter(null);
    history.log("Remove filtered-out rows", `${hidden.length} rows`, null, () =>
      file.setRemovedRowIndices(prevRemoved),
    );
  };
  const handleFile = async (uploaded) => {
    setSelectedColumn(null);
    setActiveDataset("original");
    setSynthDialogOpen(false);
    setCompareOpen(false);
    original.rowFilter.setRowFilter(null);
    synthetic.rowFilter.setRowFilter(null);
    anonymize.reset();
    synthesis.reset();
    const loaded = await file.loadFile(uploaded);
    if (loaded) {
      history.log(
        "Load dataset",
        `${loaded.rowCount} rows, ${loaded.headers.length} cols`,
        null,
        () => {
          file.reset();
          anonymize.reset();
          synthesis.reset();
          setActiveDataset("original");
          original.rowFilter.setRowFilter(null);
          history.clear();
        },
      );
    }
  };

  const error = file.error || anonymize.error;

  return (
    <div className="app-shell" data-dataset={shownKey}>
      <Navbar
        onFileSelected={handleFile}
        onDownloadContract={file.downloadContract}
        onDownloadRecipe={file.downloadRecipe}
        onDownloadData={anonymize.downloadData}
        canDownloadContract={!!file.contract}
        canDownloadRecipe={file.recipe.columns.length > 0}
        canDownloadData={!!anonymize.anonymizeResult || !!file.fileBytes}
        fileName={file.fileName}
        rowCount={file.sample ? file.sample.rowCount - file.removedRowIndices.size : null}
        columnCount={file.sample ? file.sample.headers.length - file.removedColumns.size : null}
        activeTier={TIER_OF_DATASET[shownKey]}
        onSelectTier={handleSelectTier}
        tier2Enabled={!!file.sample}
        syntheticActive={showingSynthetic}
        canCompare={!!syntheticResult}
        onCompare={() => setCompareOpen(true)}
        onDownloadSyntheticCsv={synthesis.downloadCsv}
        onDownloadSyntheticReport={synthesis.downloadReport}
      />
      <main className="main-content">
        {error && <div className="banner banner-error">{error}</div>}
        {file.isLoading && <LoadingOverlay label={`Reading ${file.loadingFileName}…`} />}
        {!file.isLoading && !file.sample && (
          <div className="upload-prompt">
            <Dropzone onFileSelected={handleFile} />
          </div>
        )}
        {file.sample && (
          <Suspense fallback={<LoadingOverlay label="Loading…" />}>
            <div className="dataset-view" style={{ maxWidth: tableWidth ?? undefined }}>
              <DatasetTabs
                active={shownKey}
                onSelect={selectDataset}
                hasSynthetic={!!syntheticResult}
              />
              <div className="dataset-table">
                <TableTab
                  key={showingSynthetic ? "synthetic" : "original"}
                  sample={shown.sample}
                  profile={shown.profile}
                  methods={methods}
                  recipe={showingSynthetic ? NO_RECIPE : file.recipe}
                  salt={file.salt}
                  onPickMethod={handlePickMethod}
                  removedColumns={showingSynthetic ? NO_ROWS : file.removedColumns}
                  onSaltChange={file.setSalt}
                  onAnonymize={anonymize.run}
                  anonymizing={anonymize.anonymizing}
                  anonymizeResult={showingSynthetic ? null : anonymize.anonymizeResult}
                  onColumnHover={handleColumnHover}
                  highlightColumnNames={highlightColumnNames}
                  contract={showingSynthetic ? null : file.contract}
                  onDeclareAsString={declareAsString}
                  filteredRowIndices={view.rowFilter.filteredRowIndices}
                  onClearRowFilter={() => view.rowFilter.setRowFilter(null)}
                  onRemoveVisibleRows={showingSynthetic ? undefined : handleRemoveVisibleRows}
                  onRemoveFilteredOutRows={
                    showingSynthetic ? undefined : handleRemoveFilteredOutRows
                  }
                  removedRowIndices={showingSynthetic ? NO_ROWS : file.removedRowIndices}
                  onRestoreRemovedRows={handleRestoreRemovedRows}
                  readOnly={showingSynthetic}
                  dataset={showingSynthetic ? "synthetic" : "original"}
                  onContentWidth={setTableWidth}
                />
              </div>
            </div>
            <AnalysisOverlay
              columnProfile={shown.profile}
              selectedColumn={selectedColumn}
              onSelectColumn={setSelectedColumn}
              contract={showingSynthetic ? null : file.contract}
              onCheckPattern={anonymize.checkPattern}
              patternChecks={showingSynthetic ? NO_PATTERN_CHECKS : anonymize.patternChecks}
              checkingColumns={showingSynthetic ? NO_ROWS : anonymize.checkingColumns}
              anonymizeResult={showingSynthetic ? null : anonymize.anonymizeResult}
              riskByName={view.columnRisk.riskByName}
              riskLoading={view.columnRisk.riskLoading}
              onDeclareAsString={declareAsString}
              highlightColumnNames={highlightColumnNames}
              onColumnHover={handleColumnHover}
              diversityRaw={view.diversityMatrix.raw}
              diversityLoading={view.diversityMatrix.loading}
              diversityError={view.diversityMatrix.error}
              quasiColumns={view.diversityMatrix.quasiColumns}
              allColumns={view.diversityMatrix.allColumns}
              hoverNames={hoverNames}
              onHoverNamesChange={setHoverNames}
              sample={shown.sample}
              rowRiskModel={view.diversityMatrix.rowRiskModel}
              onRowRiskModelChange={view.diversityMatrix.setRowRiskModel}
              onJointKCellClick={view.rowFilter.setRowFilter}
              methods={methods}
              recipe={showingSynthetic ? NO_RECIPE : file.recipe}
              salt={file.salt}
              onPickMethod={handlePickMethod}
              onClearMethod={handleClearMethod}
              onSaltChange={file.setSalt}
              onAnonymize={anonymize.run}
              anonymizing={anonymize.anonymizing}
              columnSubsets={view.columnSubsets}
              historyEntries={history.entries}
              onUndoHistoryEntry={history.undo}
              synthesisResult={showingSynthetic ? syntheticResult : null}
              readOnly={showingSynthetic}
              association={view.association}
              openRequest={overlayRequest}
            />
          </Suspense>
        )}
        {compareOpen && compareLeft && compareRight && (
          <Suspense fallback={null}>
            <CompareDialog
              left={compareLeft}
              right={compareRight}
              hoverNames={hoverNames}
              onHoverNamesChange={setHoverNames}
              onColumnHover={handleColumnHover}
              onClose={() => setCompareOpen(false)}
            />
          </Suspense>
        )}
        {synthDialogOpen && (
          <SynthesizeDialog
            synthesis={synthesis}
            fileName={file.fileName}
            onClose={() => setSynthDialogOpen(false)}
            onDone={handleSynthesisDone}
          />
        )}
      </main>
    </div>
  );
}
