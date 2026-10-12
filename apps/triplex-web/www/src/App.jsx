import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { useWasm } from "./hooks/useWasm";
import { useProfiledFile } from "./hooks/useProfiledFile";
import { useAnonymize } from "./hooks/useAnonymize";
import { useDatasetAnalysis } from "./hooks/useDatasetAnalysis";
import { useHistory } from "./hooks/useHistory";
import { useSynthesize } from "./hooks/useSynthesize";
import { useSketch } from "./hooks/useSketch";
import { withColumnType } from "./state/yamlBuilder";
import Navbar from "./components/Navbar";
import DatasetTabs from "./components/DatasetTabs";
import SynthesizeDialog from "./components/SynthesizeDialog";
import SketchDialog from "./components/SketchDialog";
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

// What a read-only generated view (synthetic or sketched) passes where the original view passes its anonymization state:
// the synthetic table has no recipe, contract, pattern checks or removed rows of its own.
const NO_ROWS = new Set();
const NO_RECIPE = { columns: [] };
const NO_PATTERN_CHECKS = {};
const TIER_OF_DATASET = { original: 1, synthetic: 2, sketched: 3 };

/** Composition root only: wires the three hooks together and renders. Every
 * actual behavior (wasm lifecycle, file/contract/recipe state, the
 * anonymize/pattern-check workflow) lives in its own hook under `hooks/` —
 * keeping that logic out of this file is deliberate, not an oversight.
 *
 * Both props are for a host that embeds the app (the Jupyter widget, `widget.jsx`); the
 * standalone page passes neither. `initialFile` is a `File` loaded as if the user had dropped
 * it. `onHostState` is called with the artifacts the user has produced so far, whenever one of
 * them changes. */
export default function App({ initialFile = null, onHostState = null }) {
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
  // Tier 3: the sketch and the table generated from it.
  const sketching = useSketch({
    fileBytes: file.fileBytes,
    extension: file.extension,
    removedRowIndices: file.removedRowIndices,
    removedColumns: file.removedColumns,
  });
  const sketchedResult = sketching.result;
  const sketched = useDatasetAnalysis(
    wasm,
    sketchedResult?.sample,
    sketchedResult?.profile,
    NO_ROWS,
  );
  // Which dataset the Table and the panel show. A generated dataset (synthetic, Tier 2, or
  // sketched, Tier 3) is read-only: no recipe, contract or row removal, and its analysis tabs
  // describe the generated table.
  const [activeDataset, setActiveDataset] = useState("original");
  const generatedDatasets = {
    synthetic: {
      key: "synthetic",
      label: "Synthetic",
      result: syntheticResult,
      analysis: synthetic,
    },
    sketched: { key: "sketched", label: "Sketched", result: sketchedResult, analysis: sketched },
  };
  const generated = generatedDatasets[activeDataset]?.result
    ? generatedDatasets[activeDataset]
    : null;
  const showingGenerated = !!generated;
  const shownKey = generated?.key ?? "original";
  const [compareOpen, setCompareOpen] = useState(false);
  const view = generated?.analysis ?? original;
  const shown = generated?.result ?? file;
  // What Compare puts next to the original: the generated dataset on screen, else whichever exists.
  const compared =
    generated ??
    (syntheticResult ? generatedDatasets.synthetic : null) ??
    (sketchedResult ? generatedDatasets.sketched : null);
  const comparedResult = compared?.result;
  const comparedAnalysis = compared?.analysis;
  const comparedLabel = compared?.label;
  const comparedKey = compared?.key;
  // The Compare view's two sides: the original (minus columns the user removed) and a generated
  // table. A later uploaded dataset becomes another right-hand side here.
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
      comparedResult
        ? {
            label: comparedLabel,
            kind: comparedKey,
            sample: comparedResult.sample,
            profile: comparedResult.profile,
            analysis: comparedAnalysis,
            excluded: NO_ROWS,
            droppedReasons: Object.fromEntries(
              (comparedResult.report.columns_dropped ?? []).map((d) => [d.column, d.reason]),
            ),
          }
        : null,
    [comparedResult, comparedAnalysis, comparedLabel, comparedKey],
  );
  const [synthDialogOpen, setSynthDialogOpen] = useState(false);
  const [sketchDialogOpen, setSketchDialogOpen] = useState(false);
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
  // Tier 1 shows the original; Tier 2 opens the synthesis dialog and Tier 3 the sketch dialog
  // (also to re-run them).
  const handleSelectTier = (tier) => {
    if (tier === 1) selectDataset("original");
    if (tier === 2) setSynthDialogOpen(true);
    if (tier === 3) setSketchDialogOpen(true);
  };
  const handleSketchDone = () => {
    setSketchDialogOpen(false);
    selectDataset("sketched");
    setOverlayRequest((r) => ({ tab: "analysis", seq: r.seq + 1 }));
    history.log(
      "Generate sketched data",
      `${sketching.sketch?.classes.length ?? 0} classes`,
      null,
      () => {
        sketching.clearResult();
        selectDataset("original");
      },
    );
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
    setSketchDialogOpen(false);
    setCompareOpen(false);
    original.rowFilter.setRowFilter(null);
    synthetic.rowFilter.setRowFilter(null);
    sketched.rowFilter.setRowFilter(null);
    anonymize.reset();
    synthesis.reset();
    sketching.reset();
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
          sketching.reset();
          setActiveDataset("original");
          original.rowFilter.setRowFilter(null);
          history.clear();
        },
      );
    }
  };

  useEffect(() => {
    if (initialFile) handleFile(initialFile);
    // handleFile is a new function on every render; only a new file from the host re-runs this.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialFile]);

  // What a host can read back. The mapping file reverses the pseudonyms: it is reported so the
  // host can offer it, and the host decides whether it goes any further (widget.jsx sends it to
  // the kernel only when the user opted in). The salt is deliberately not here.
  const anonymizeResult = anonymize.anonymizeResult;
  useEffect(() => {
    if (!onHostState || !wasm) return;
    onHostState({
      recipe_yaml:
        file.recipe.columns.length > 0 ? wasm.recipe_json_to_yaml(JSON.stringify(file.recipe)) : "",
      contract_yaml: file.contract ? wasm.contract_json_to_yaml(JSON.stringify(file.contract)) : "",
      masked_csv: anonymizeResult?.masked_csv ?? null,
      anonymize_report_json: anonymizeResult?.report_json ?? "",
      mapping_csv: anonymizeResult?.mapping_csv ?? null,
      synthetic_csv: syntheticResult?.csv ?? null,
      synthesis_report_json: syntheticResult?.reportJson ?? "",
    });
  }, [onHostState, wasm, file.recipe, file.contract, anonymizeResult, syntheticResult]);

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
        syntheticActive={showingGenerated}
        generatedNoun={shownKey === "sketched" ? "sketched" : "synthetic"}
        canCompare={!!compared}
        onCompare={() => setCompareOpen(true)}
        onDownloadSyntheticCsv={
          shownKey === "sketched" ? sketching.downloadCsv : synthesis.downloadCsv
        }
        onDownloadSyntheticReport={
          shownKey === "sketched" ? sketching.downloadReport : synthesis.downloadReport
        }
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
                hasSketched={!!sketchedResult}
              />
              <div className="dataset-table">
                <TableTab
                  key={shownKey}
                  sample={shown.sample}
                  profile={shown.profile}
                  methods={methods}
                  recipe={showingGenerated ? NO_RECIPE : file.recipe}
                  salt={file.salt}
                  onPickMethod={handlePickMethod}
                  removedColumns={showingGenerated ? NO_ROWS : file.removedColumns}
                  onSaltChange={file.setSalt}
                  onAnonymize={anonymize.run}
                  anonymizing={anonymize.anonymizing}
                  anonymizeResult={showingGenerated ? null : anonymize.anonymizeResult}
                  onColumnHover={handleColumnHover}
                  highlightColumnNames={highlightColumnNames}
                  contract={showingGenerated ? null : file.contract}
                  onDeclareAsString={declareAsString}
                  filteredRowIndices={view.rowFilter.filteredRowIndices}
                  onClearRowFilter={() => view.rowFilter.setRowFilter(null)}
                  onRemoveVisibleRows={showingGenerated ? undefined : handleRemoveVisibleRows}
                  onRemoveFilteredOutRows={
                    showingGenerated ? undefined : handleRemoveFilteredOutRows
                  }
                  removedRowIndices={showingGenerated ? NO_ROWS : file.removedRowIndices}
                  onRestoreRemovedRows={handleRestoreRemovedRows}
                  readOnly={showingGenerated}
                  dataset={shownKey}
                  onContentWidth={setTableWidth}
                />
              </div>
            </div>
            <AnalysisOverlay
              columnProfile={shown.profile}
              selectedColumn={selectedColumn}
              onSelectColumn={setSelectedColumn}
              contract={showingGenerated ? null : file.contract}
              onCheckPattern={anonymize.checkPattern}
              patternChecks={showingGenerated ? NO_PATTERN_CHECKS : anonymize.patternChecks}
              checkingColumns={showingGenerated ? NO_ROWS : anonymize.checkingColumns}
              anonymizeResult={showingGenerated ? null : anonymize.anonymizeResult}
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
              recipe={showingGenerated ? NO_RECIPE : file.recipe}
              salt={file.salt}
              onPickMethod={handlePickMethod}
              onClearMethod={handleClearMethod}
              onSaltChange={file.setSalt}
              onAnonymize={anonymize.run}
              anonymizing={anonymize.anonymizing}
              columnSubsets={view.columnSubsets}
              historyEntries={history.entries}
              onUndoHistoryEntry={history.undo}
              synthesisResult={generated?.result ?? null}
              generatedLabel={generated?.label}
              readOnly={showingGenerated}
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
        {sketchDialogOpen && (
          <SketchDialog
            sketching={sketching}
            original={file.sample}
            excluded={file.removedRowIndices}
            fileName={file.fileName}
            onClose={() => setSketchDialogOpen(false)}
            onDone={handleSketchDone}
          />
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
