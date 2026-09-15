import { lazy, Suspense, useCallback, useState } from "react";
import { useWasm } from "./hooks/useWasm";
import { useProfiledFile } from "./hooks/useProfiledFile";
import { useAnonymize } from "./hooks/useAnonymize";
import { useColumnRisk } from "./hooks/useColumnRisk";
import { useDiversityMatrix } from "./hooks/useDiversityMatrix";
import { useRowFilter } from "./hooks/useRowFilter";
import { withColumnType } from "./state/yamlBuilder";
import Navbar from "./components/Navbar";
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
  });
  const columnRisk = useColumnRisk(file.sample, file.profile);
  const diversityMatrix = useDiversityMatrix(file.sample, file.profile);
  const rowFilter = useRowFilter(wasm, file.sample, file.profile);
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
  const handleColumnHover = useCallback((name) => {
    setHoverNames(name ? { rowName: name, colName: name } : null);
  }, []);
  const declareAsString = (name) => file.setContract((c) => c && withColumnType(c, name, "string"));
  const handleFile = async (uploaded) => {
    setSelectedColumn(null);
    rowFilter.setRowFilter(null);
    anonymize.reset();
    await file.loadFile(uploaded);
  };

  const error = file.error || anonymize.error;

  return (
    <div className="app-shell">
      <Navbar
        onFileSelected={handleFile}
        onDownloadContract={file.downloadContract}
        onDownloadRecipe={file.downloadRecipe}
        onDownloadData={anonymize.downloadData}
        canDownloadContract={!!file.contract}
        canDownloadRecipe={file.recipe.columns.length > 0}
        canDownloadData={!!anonymize.anonymizeResult || !!file.fileBytes}
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
            <TableTab
              sample={file.sample}
              profile={file.profile}
              methods={methods}
              recipe={file.recipe}
              salt={file.salt}
              onPickMethod={file.pickMethod}
              removedColumns={file.removedColumns}
              onRemoveColumn={file.removeColumn}
              onSaltChange={file.setSalt}
              onAnonymize={anonymize.run}
              anonymizing={anonymize.anonymizing}
              anonymizeResult={anonymize.anonymizeResult}
              onColumnHover={handleColumnHover}
              highlightColumnNames={highlightColumnNames}
              contract={file.contract}
              onDeclareAsString={declareAsString}
              filteredRowIndices={rowFilter.filteredRowIndices}
              onClearRowFilter={() => rowFilter.setRowFilter(null)}
            />
            <AnalysisOverlay
              columnProfile={file.profile}
              selectedColumn={selectedColumn}
              onSelectColumn={setSelectedColumn}
              contract={file.contract}
              onCheckPattern={anonymize.checkPattern}
              patternChecks={anonymize.patternChecks}
              checkingColumns={anonymize.checkingColumns}
              anonymizeResult={anonymize.anonymizeResult}
              riskByName={columnRisk.riskByName}
              riskLoading={columnRisk.riskLoading}
              onDeclareAsString={declareAsString}
              highlightColumnNames={highlightColumnNames}
              onColumnHover={handleColumnHover}
              diversityRaw={diversityMatrix.raw}
              diversityLoading={diversityMatrix.loading}
              diversityError={diversityMatrix.error}
              quasiColumns={diversityMatrix.quasiColumns}
              allColumns={diversityMatrix.allColumns}
              hoverNames={hoverNames}
              onHoverNamesChange={setHoverNames}
              sample={file.sample}
              rowRiskModel={diversityMatrix.rowRiskModel}
              onRowRiskModelChange={diversityMatrix.setRowRiskModel}
              onJointKCellClick={rowFilter.setRowFilter}
              methods={methods}
              recipe={file.recipe}
              salt={file.salt}
              onPickMethod={file.pickMethod}
              onClearMethod={file.clearMethod}
              onRemoveColumn={file.removeColumn}
              onSaltChange={file.setSalt}
              onAnonymize={anonymize.run}
              anonymizing={anonymize.anonymizing}
            />
          </Suspense>
        )}
      </main>
    </div>
  );
}
