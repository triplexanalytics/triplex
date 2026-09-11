import TierPicker from "./TierPicker";
import { DOWNLOAD_FORMATS } from "../utils";

export default function Navbar({
  onFileSelected,
  onDownloadContract,
  onDownloadRecipe,
  onDownloadData,
  downloadFormat,
  onDownloadFormatChange,
  canDownloadContract,
  canDownloadRecipe,
  canDownloadData,
}) {
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
      <select
        aria-label="Download format"
        value={downloadFormat}
        onChange={(e) => onDownloadFormatChange(e.target.value)}
        disabled={!canDownloadData}
      >
        <option value="">Original format</option>
        {DOWNLOAD_FORMATS.map((ext) => (
          <option key={ext} value={ext}>
            .{ext}
          </option>
        ))}
      </select>
      <button
        className={canDownloadData ? "primary" : undefined}
        onClick={onDownloadData}
        disabled={!canDownloadData}
      >
        Download data
      </button>
      {/* Unstyled placeholder only — no gating logic. */}
      <span className="license-placeholder">License: Pro</span>
    </div>
  );
}
