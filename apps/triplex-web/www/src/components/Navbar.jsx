import TierPicker from "./TierPicker";

export default function Navbar({
  onFileSelected,
  onDownloadContract,
  onDownloadRecipe,
  onDownloadData,
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
      <button onClick={onDownloadData} disabled={!canDownloadData}>
        Download data
      </button>
      {/* Unstyled placeholder only — no gating logic. */}
      <span className="license-placeholder">License: Pro</span>
    </div>
  );
}
