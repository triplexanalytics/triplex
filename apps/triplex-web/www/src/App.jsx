import { lazy, Suspense, useCallback, useState } from "react";
import { useWasm } from "./hooks/useWasm";
import { useProfiledFile } from "./hooks/useProfiledFile";
import { useAnonymize } from "./hooks/useAnonymize";
import { useColumnRisk } from "./hooks/useColumnRisk";
import { useDiversityMatrix } from "./hooks/useDiversityMatrix";
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
  const [selectedColumn, setSelectedColumn] = useState(null);
  // { rowName, colName } | null — owned here rather than inside
  // DiversityCheckSection, since RiskMatrix cells cross-highlight each
  // other across every matrix in the Analysis inner tab (AnalysisOverlay),
  // not just within one. Also driven by a table header hover (see
  // handleColumnHover below), not just a matrix cell's own hover.
  const [hoverNames, setHoverNames] = useState(null);
  // Column name | null — owned here (not TableTab or AnalysisOverlay):
  // hovering a table header highlights the matching ColumnCard in
  // AnalysisOverlay's Columns tab. One-directional only (a ColumnCard hover
  // doesn't highlight the table back) — see useTableHeaderCanvas.js's doc
  // comment for why the reverse direction isn't built.
  const [hoveredColumnName, setHoveredColumnName] = useState(null);
  // A table header hover drives both cross-highlights at once: the matching
  // ColumnCard (hoveredColumnName) and the matching row/column band across
  // every RiskMatrix in the Analysis tab (hoverNames) — the same {rowName,
  // colName} shape a matrix cell's own hover already sets, matched
  // independently per axis so this lights up every matrix this column
  // appears in, on whichever side it appears.
  const handleColumnHover = useCallback((name) => {
    setHoveredColumnName(name);
    setHoverNames(name ? { rowName: name, colName: name } : null);
  }, []);
  const declareAsString = (name) => file.setContract((c) => c && withColumnType(c, name, "string"));
  const handleFile = async (uploaded) => {
    setSelectedColumn(null);
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
              riskByName={columnRisk.riskByName}
              riskLoading={columnRisk.riskLoading}
              onColumnHover={handleColumnHover}
              contract={file.contract}
              onDeclareAsString={declareAsString}
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
              highlightColumnName={hoveredColumnName}
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
            />
          </Suspense>
        )}
      </main>
    </div>
  );
}
