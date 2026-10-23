import { useEffect, useRef, useState } from "react";
import {
  ChevronDown,
  Download,
  FileCheck,
  FileDown,
  FileUp,
  FolderOpen,
  GitCompareArrows,
} from "lucide-react";
import TierPicker from "./TierPicker";
import { DOWNLOAD_FORMATS } from "../utils";
import { RECIPE_FILE_EXTENSIONS } from "../state/recipeFile";

// Icons are decoration next to a text label, so screen readers skip them.
const ICON = { size: 16, strokeWidth: 2, "aria-hidden": true };
// How long the dot next to Compare datasets pulses after a new anonymize result.
const PULSE_MS = 10_000;

/** `fileChooser`, when given, takes the place of the "Choose file" button and its file input.
 * `comparePulse` is the newest result Compare can show (or null): each new one makes a dot next
 * to Compare datasets pulse for ten seconds, or until Compare is opened. */
export default function Navbar({
  onFileSelected,
  fileChooser = null,
  onDownloadContract,
  onDownloadRecipe,
  onOpenRecipe,
  onDownloadData,
  onDownloadMapping,
  canDownloadMapping = false,
  canDownloadContract,
  canDownloadRecipe,
  canOpenRecipe = false,
  canDownloadData,
  fileName,
  rowCount,
  columnCount,
  activeTier,
  onSelectTier,
  tierDisabledReasons,
  proTiers,
  edition = "Free",
  syntheticActive = false,
  generatedNoun = "synthetic",
  canCompare = false,
  compareDisabledReason,
  comparePulse = null,
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
  const recipeInputRef = useRef(null);

  const [pulsing, setPulsing] = useState(false);
  useEffect(() => {
    if (!comparePulse) {
      setPulsing(false);
      return;
    }
    setPulsing(true);
    const timer = setTimeout(() => setPulsing(false), PULSE_MS);
    return () => clearTimeout(timer);
  }, [comparePulse]);

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
      {fileChooser ?? (
        <>
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
          <button type="button" className="op-file" onClick={() => fileInputRef.current?.click()}>
            <FolderOpen {...ICON} />
            Choose file
          </button>
        </>
      )}
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
      {/* The operation bar: what can be done with the loaded table, grouped by what it acts on. */}
      <div className="op-bar" role="toolbar" aria-label="Operations">
        {syntheticActive ? (
          <div className="op-group">
            <details className="download-menu" ref={detailsRef}>
              <summary className="primary">
                <Download {...ICON} />
                Download {generatedNoun} data
                <ChevronDown {...ICON} />
              </summary>
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
          </div>
        ) : (
          <div className="op-group" role="group" aria-label="Recipe and contract">
            {/* A recipe is configuration, not data, so it can be opened wherever the app runs,
                  including builds that replace the data file chooser. */}
            <input
              ref={recipeInputRef}
              type="file"
              accept={RECIPE_FILE_EXTENSIONS.map((ext) => `.${ext}`).join(",")}
              hidden
              onChange={(e) => {
                const chosen = e.target.files[0];
                // Cleared so choosing the same file again (after editing it) opens it again.
                e.target.value = "";
                if (chosen) onOpenRecipe(chosen);
              }}
            />
            <button
              type="button"
              onClick={() => recipeInputRef.current?.click()}
              disabled={!canOpenRecipe}
              title={canOpenRecipe ? undefined : "Load a data file first"}
            >
              <FileUp {...ICON} />
              Open recipe
            </button>
            <button type="button" onClick={onDownloadRecipe} disabled={!canDownloadRecipe}>
              <FileDown {...ICON} />
              Download recipe
            </button>
            <button type="button" onClick={onDownloadContract} disabled={!canDownloadContract}>
              <FileCheck {...ICON} />
              Download contract
            </button>
          </div>
        )}
        <div className="op-group">
          {/* Compare sets the anonymized output or a generated table beside the original. */}
          <button
            type="button"
            className="op-compare"
            onClick={() => {
              setPulsing(false);
              onCompare();
            }}
            disabled={!canCompare}
            title={canCompare ? undefined : compareDisabledReason}
          >
            <GitCompareArrows {...ICON} />
            Compare datasets
            {pulsing && <span className="pulse-dot" aria-hidden="true" />}
          </button>
        </div>
        <div className="op-end">
          {!syntheticActive &&
            (canDownloadData ? (
              <details className="download-menu" ref={detailsRef}>
                <summary className="primary">
                  <Download {...ICON} />
                  Download data
                  <ChevronDown {...ICON} />
                </summary>
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
              <button type="button" className="primary" disabled>
                <Download {...ICON} />
                Download data
              </button>
            ))}
          {/* A label for the build that is running, not a license check. */}
          <span className="license">License: {edition}</span>
        </div>
      </div>
    </div>
  );
}
