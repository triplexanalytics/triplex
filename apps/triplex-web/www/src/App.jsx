import { useState } from "react";
import { useWasm } from "./hooks/useWasm";
import { useProfiledFile } from "./hooks/useProfiledFile";
import { useAnonymize } from "./hooks/useAnonymize";
import Navbar from "./components/Navbar";
import Tabs from "./components/Tabs";
import TableTab from "./components/TableTab";
import ColumnViewTab from "./components/ColumnViewTab";
import Dropzone from "./components/Dropzone";

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
  const [activeTab, setActiveTab] = useState("table");
  const [selectedColumn, setSelectedColumn] = useState(null);

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
        {!file.sample && (
          <div className="upload-prompt">
            <Dropzone onFileSelected={handleFile} />
          </div>
        )}
        {file.sample && activeTab === "table" && (
          <TableTab
            sample={file.sample}
            profile={file.profile}
            methods={methods}
            recipe={file.recipe}
            salt={file.salt}
            onPickMethod={file.pickMethod}
            onSaltChange={file.setSalt}
            onAnonymize={anonymize.run}
            anonymizeResult={anonymize.anonymizeResult}
          />
        )}
        {file.sample && activeTab === "column" && (
          <ColumnViewTab
            profile={file.profile}
            selectedColumn={selectedColumn}
            onSelectColumn={setSelectedColumn}
            contract={file.contract}
            onCheckPattern={anonymize.checkPattern}
            patternChecks={anonymize.patternChecks}
            anonymizeResult={anonymize.anonymizeResult}
          />
        )}
      </main>
    </div>
  );
}
