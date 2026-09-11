import { useRef } from "react";
import TierPicker from "./TierPicker";
import { DOWNLOAD_FORMATS } from "../utils";

export default function Navbar({
  onFileSelected,
  onDownloadContract,
  onDownloadRecipe,
  onDownloadData,
  canDownloadContract,
  canDownloadRecipe,
  canDownloadData,
}) {
  // Native <details>/<summary> instead of a hand-rolled open/close state +
  // click-outside listener — the one thing it doesn't give for free is
  // closing itself after a menu item is picked, so `pick` does that via
  // the ref. ponytail: it also doesn't close on an outside click the way a
  // JS-driven menu would; clicking the button again (or picking a format)
  // closes it, which covers the actual use here.
  const detailsRef = useRef(null);

  const pick = (format) => {
    if (detailsRef.current) detailsRef.current.open = false;
    onDownloadData(format);
  };

  return (
    <div className="navbar">
      <span className="brand">Triplex</span>
      <input
        type="file"
        accept=".csv,.tsv,.jsonl"
        onChange={(e) => e.target.files[0] && onFileSelected(e.target.files[0])}
      />
      <TierPicker />
      <div className="spacer" />
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
          </ul>
        </details>
      ) : (
        <button disabled>Download data</button>
      )}
      {/* Unstyled placeholder only — no gating logic. */}
      <span className="license-placeholder">License: Pro</span>
    </div>
  );
}
