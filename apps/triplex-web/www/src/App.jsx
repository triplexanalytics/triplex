import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { useWasm } from "./hooks/useWasm";
import { useProfiledFile } from "./hooks/useProfiledFile";
import { useAnonymize } from "./hooks/useAnonymize";
import { useDatasetAnalysis } from "./hooks/useDatasetAnalysis";
import { useHistory } from "./hooks/useHistory";
import { withColumnType } from "./state/yamlBuilder";
import Navbar from "./components/Navbar";
import DatasetTabs from "./components/DatasetTabs";
import Dropzone from "./components/Dropzone";
import LoadingOverlay from "./components/LoadingOverlay";
import { EXTENSIONS } from "./extensions";
import { tierDisabledReason } from "./extensionApi";

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

/** Composition root only: wires the three hooks together and renders. Every
 * actual behavior (wasm lifecycle, file/contract/recipe state, the
 * anonymize/pattern-check workflow) lives in its own hook under `hooks/` —
 * keeping that logic out of this file is deliberate, not an oversight.
 *
 * Both props are for a host that embeds the app (the Jupyter widget, `widget.jsx`); the
 * standalone page passes neither. `initialFile` is a `File` loaded as if the user had dropped
 * it. `onHostState` is called with the artifacts the user has produced so far, whenever one of
 * them changes. `extensions` defaults to this build's generator extensions (`extensions.js`);
 * tests pass their own. It must not change while the app is mounted. */
export default function App({ initialFile = null, onHostState = null, extensions = EXTENSIONS }) {
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
  const history = useHistory();
  // What every generator extension sees of the loaded file.
  const host = {
    fileBytes: file.fileBytes,
    fileExtension: file.extension,
    fileName: file.fileName,
    sample: file.sample,
    removedRowIndices: file.removedRowIndices,
    removedColumns: file.removedColumns,
  };
  // One generator per extension (Tier 2 synthesis, Tier 3 sketching), each with the analysis of
  // the table it generated; idle until a run exists (the analysis hooks accept a null sample).
  // The list is fixed for the app's lifetime, so these hooks run in the same order on every
  // render, which is all React's rules of hooks require.
  const generators = extensions.map((ext) => {
    const generator = ext.useGenerator(host);
    // eslint-disable-next-line react-hooks/rules-of-hooks -- fixed list, see above
    const analysis = useDatasetAnalysis(
      wasm,
      generator.result?.sample,
      generator.result?.profile,
      NO_ROWS,
    );
    return { ext, key: ext.id, label: ext.label, generator, result: generator.result, analysis };
  });
  const generatorById = Object.fromEntries(generators.map((g) => [g.key, g]));
  // Which dataset the Table and the panel show. A generated dataset is read-only: no recipe,
  // contract or row removal, and its analysis tabs describe the generated table.
  const [activeDataset, setActiveDataset] = useState("original");
  const generated = generatorById[activeDataset]?.result ? generatorById[activeDataset] : null;
  const showingGenerated = !!generated;
  const shownKey = generated?.key ?? "original";
  const [compareOpen, setCompareOpen] = useState(false);
  const view = generated?.analysis ?? original;
  const shown = generated?.result ?? file;
  // What Compare puts next to the original: the generated dataset on screen, else whichever exists.
  const compared = generated ?? generators.find((g) => g.result) ?? null;
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
  // The id of the extension whose dialog is open, or null.
  const [openDialog, setOpenDialog] = useState(null);
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
  // Tier 1 shows the original; a higher tier opens the dialog of its extension (also to re-run
  // it).
  const handleSelectTier = (tier) => {
    if (tier === 1) selectDataset("original");
    const ext = extensions.find((e) => e.tier === tier);
    if (ext) setOpenDialog(ext.id);
  };
  const handleGeneratorDone = (g) => {
    setOpenDialog(null);
    selectDataset(g.key);
    setOverlayRequest((r) => ({ tab: "analysis", seq: r.seq + 1 }));
    history.log(g.ext.history.label, g.ext.history.detail(g.generator), null, () => {
      g.generator.clearResult();
      selectDataset("original");
    });
  };
  const dialogGenerator = openDialog ? generatorById[openDialog] : null;
  // Generator tiers this build has no extension for are shown as links to Triplex Pro.
  const proTiers = [2, 3].filter((tier) => !extensions.some((e) => e.tier === tier));
  const tierDisabledReasons = Object.fromEntries(
    extensions.map((ext) => [ext.tier, tierDisabledReason(ext, host)]),
  );
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
    setOpenDialog(null);
    setCompareOpen(false);
    original.rowFilter.setRowFilter(null);
    anonymize.reset();
    for (const g of generators) {
      g.analysis.rowFilter.setRowFilter(null);
      g.generator.reset();
    }
    const loaded = await file.loadFile(uploaded);
    if (loaded) {
      history.log(
        "Load dataset",
        `${loaded.rowCount} rows, ${loaded.headers.length} cols`,
        null,
        () => {
          file.reset();
          anonymize.reset();
          for (const g of generators) g.generator.reset();
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
  const generatedResults = generators.map((g) => g.result);
  useEffect(() => {
    if (!onHostState || !wasm) return;
    onHostState({
      recipe_yaml:
        file.recipe.columns.length > 0 ? wasm.recipe_json_to_yaml(JSON.stringify(file.recipe)) : "",
      contract_yaml: file.contract ? wasm.contract_json_to_yaml(JSON.stringify(file.contract)) : "",
      masked_csv: anonymizeResult?.masked_csv ?? null,
      anonymize_report_json: anonymizeResult?.report_json ?? "",
      mapping_csv: anonymizeResult?.mapping_csv ?? null,
      ...Object.assign({}, ...generators.map((g) => g.ext.hostState?.(g.result) ?? {})),
    });
    // The generators are read through their results, listed one by one: the list has a fixed
    // length, so the dependency array does too.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onHostState, wasm, file.recipe, file.contract, anonymizeResult, ...generatedResults]);

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
        activeTier={generated?.ext.tier ?? 1}
        onSelectTier={handleSelectTier}
        tierDisabledReasons={tierDisabledReasons}
        proTiers={proTiers}
        canGenerate={generators.length > 0}
        syntheticActive={showingGenerated}
        generatedNoun={generated?.ext.noun ?? "synthetic"}
        canCompare={!!compared}
        onCompare={() => setCompareOpen(true)}
        onDownloadSyntheticCsv={generated?.generator.downloadCsv}
        onDownloadSyntheticReport={generated?.generator.downloadReport}
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
              {generators.length > 0 && (
                <DatasetTabs
                  active={shownKey}
                  onSelect={selectDataset}
                  datasets={[
                    { id: "original", label: "Original", disabledReason: null },
                    ...generators.map((g) => ({
                      id: g.key,
                      label: g.label,
                      disabledReason: g.result ? null : g.ext.emptyHint,
                    })),
                  ]}
                />
              )}
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
              GeneratedReport={generated?.ext.Report}
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
        {dialogGenerator && (
          <dialogGenerator.ext.Dialog
            generator={dialogGenerator.generator}
            host={host}
            onClose={() => setOpenDialog(null)}
            onDone={() => handleGeneratorDone(dialogGenerator)}
          />
        )}
      </main>
    </div>
  );
}
