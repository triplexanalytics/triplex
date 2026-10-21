import { useRef } from "react";
import TierPicker from "./TierPicker";
import { DOWNLOAD_FORMATS } from "../utils";

export default function Navbar({
  onFileSelected,
  onDownloadContract,
  onDownloadRecipe,
  onDownloadData,
  onDownloadMapping,
  canDownloadMapping = false,
  canDownloadContract,
  canDownloadRecipe,
  canDownloadData,
  fileName,
  rowCount,
  columnCount,
  activeTier,
  onSelectTier,
  tierDisabledReasons,
  proTiers,
  canGenerate = true,
  edition = "Free",
  syntheticActive = false,
  generatedNoun = "synthetic",
  canCompare = false,
  onCompare,
  onDownloadSyntheticCsv,
  onDownloadSyntheticReport,
}) {
  // Native <details>/<summary> instead of a hand-rolled open/close state +
  // click-outside listener — the one thing it doesn't give for free is
  // closing itself after a menu item is picked, so `pick` does that via
  // the ref. ponytail: it also doesn't close on an outside click the way a
  // JS-driven menu would; clicking the button again (or picking a format)
  // closes it, which covers the actual use here.
  const detailsRef = useRef(null);
  const fileInputRef = useRef(null);

  const pick = (format) => {
    if (detailsRef.current) detailsRef.current.open = false;
    onDownloadData(format);
  };

  return (
    <div className="navbar">
      <span className="brand">Triplex</span>
      {import.meta.env.VITE_APP_VERSION && (
        <span className="version">v{import.meta.env.VITE_APP_VERSION}</span>
      )}
      {/* Hidden and driven by our own `fileName` state, not the browser's
          native file-input label -- a file loaded via Dropzone (drag or its
          own click-to-browse) never touches this input, so the native label
          would show stale text or "No file chosen" after that path. */}
      <input
        ref={fileInputRef}
        type="file"
        accept=".csv,.tsv,.jsonl"
        hidden
        onChange={(e) => e.target.files[0] && onFileSelected(e.target.files[0])}
      />
      <button type="button" onClick={() => fileInputRef.current?.click()}>
        Choose file
      </button>
      <span className="file-shape">
        {fileName || "No file chosen"}
        {rowCount !== null &&
          ` (${rowCount.toLocaleString()} rows / ${columnCount.toLocaleString()} columns)`}
      </span>
      <TierPicker
        activeTier={activeTier}
        onSelectTier={onSelectTier}
        disabledReasons={tierDisabledReasons}
        proTiers={proTiers}
      />
      <div className="spacer" />
      {/* Compare sets a generated table beside the original; without a generator there is none. */}
      {canGenerate && (
        <button
          type="button"
          onClick={onCompare}
          disabled={!canCompare}
          title={canCompare ? undefined : "Generate synthetic data first (Tier 2 card)"}
        >
          Compare datasets
        </button>
      )}
      {syntheticActive ? (
        <details className="download-menu" ref={detailsRef}>
          <summary className="primary">Download {generatedNoun} data</summary>
          <ul>
            <li>
              <button
                type="button"
                onClick={() => {
                  detailsRef.current.open = false;
                  onDownloadSyntheticCsv();
                }}
              >
                {generatedNoun === "sketched" ? "Sketched" : "Synthetic"} data (.csv)
              </button>
            </li>
            <li>
              <button
                type="button"
                onClick={() => {
                  detailsRef.current.open = false;
                  onDownloadSyntheticReport();
                }}
              >
                Synthesis report (.json)
              </button>
            </li>
          </ul>
        </details>
      ) : (
        <>
          <button onClick={onDownloadContract} disabled={!canDownloadContract}>
            Download contract
          </button>
          <button onClick={onDownloadRecipe} disabled={!canDownloadRecipe}>
            Download recipe
          </button>
          {canDownloadData ? (
            <details className="download-menu" ref={detailsRef}>
              <summary className="primary">Download data</summary>
              <ul>
                <li>
                  <button type="button" onClick={() => pick("")}>
                    Original format
                  </button>
                </li>
                {DOWNLOAD_FORMATS.map((ext) => (
                  <li key={ext}>
                    <button type="button" onClick={() => pick(ext)}>
                      .{ext}
                    </button>
                  </li>
                ))}
                {canDownloadMapping && (
                  <li className="download-menu-separate">
                    <button
                      type="button"
                      title="Reverses the masking. Keep it apart from the masked data."
                      onClick={() => {
                        detailsRef.current.open = false;
                        onDownloadMapping();
                      }}
                    >
                      Mapping file (.csv)
                    </button>
                  </li>
                )}
              </ul>
            </details>
          ) : (
            <button disabled>Download data</button>
          )}
        </>
      )}
      {/* A label for the build that is running, not a license check. */}
      <span className="license">License: {edition}</span>
    </div>
  );
}
