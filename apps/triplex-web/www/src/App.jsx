import { lazy, Suspense, useCallback, useState } from "react";
import { useWasm } from "./hooks/useWasm";
import { useProfiledFile } from "./hooks/useProfiledFile";
import { useAnonymize } from "./hooks/useAnonymize";
import { useColumnRisk } from "./hooks/useColumnRisk";
import { useDiversityMatrix } from "./hooks/useDiversityMatrix";
import Navbar from "./components/Navbar";
import Tabs from "./components/Tabs";
import Dropzone from "./components/Dropzone";
import LoadingOverlay from "./components/LoadingOverlay";

// Code-split per tab body, not eagerly bundled with the app shell:
// TableTab alone pulls in @glideapps/glide-data-grid, and ColumnViewTab's
// sibling DiversityCheckSection pulls in the canvas risk-matrix machinery
// — neither is needed until a file is actually loaded and that tab is
// actually opened, and the Table tab (the default) still benefits from
// loading in its own chunk in parallel with the rest of the shell instead
// of being parsed/evaluated as part of one large bundle up front.
const TableTab = lazy(() => import("./components/TableTab"));
const ColumnViewTab = lazy(() => import("./components/ColumnViewTab"));
const DiversityCheckSection = lazy(() => import("./components/DiversityCheckSection"));

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
  const [activeTab, setActiveTab] = useState("table");
  const [selectedColumn, setSelectedColumn] = useState(null);
  // { rowName, colName } | null — owned here, not in DiversityCheckSection,
  // since a ColumnCard hover (ColumnViewTab, its sibling in this tab) needs
  // to reach the matrices too. A card hover sets both names to the same
  // column: RiskMatrix matches row/col names independently, so this lights
  // up that column's band on every axis it appears on across every matrix,
  // the same way hovering one matrix's own cell already lit up its siblings.
  const [hoverNames, setHoverNames] = useState(null);
  const hoverColumn = useCallback(
    (name) => setHoverNames(name ? { rowName: name, colName: name } : null),
    [],
  );

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
        canDownloadData={!!anonymize.anonymizeResult}
      />
      <Tabs active={activeTab} onChange={setActiveTab} />
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
            {activeTab === "table" && (
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
              />
            )}
            {activeTab === "column" && (
              <>
                <ColumnViewTab
                  profile={file.profile}
                  selectedColumn={selectedColumn}
                  onSelectColumn={setSelectedColumn}
                  contract={file.contract}
                  onCheckPattern={anonymize.checkPattern}
                  patternChecks={anonymize.patternChecks}
                  checkingColumns={anonymize.checkingColumns}
                  anonymizeResult={anonymize.anonymizeResult}
                  riskByName={columnRisk.riskByName}
                  riskLoading={columnRisk.riskLoading}
                  onHoverColumn={hoverColumn}
                />
                <DiversityCheckSection
                  raw={diversityMatrix.raw}
                  loading={diversityMatrix.loading}
                  error={diversityMatrix.error}
                  quasiColumns={diversityMatrix.quasiColumns}
                  allColumns={diversityMatrix.allColumns}
                  hoverNames={hoverNames}
                  onHoverNamesChange={setHoverNames}
                />
              </>
            )}
          </Suspense>
        )}
      </main>
    </div>
  );
}
