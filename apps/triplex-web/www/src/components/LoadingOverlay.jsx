/** Full-area loading state — shown while a file is being read/profiled.
 * Covers the previously-loaded table too (not just the empty-state
 * Dropzone), so re-uploading a new file over an already-loaded one doesn't
 * leave stale data on screen with no sign anything is happening. */
export default function LoadingOverlay({ label }) {
  return (
    <div className="loading-overlay">
      <div className="spinner" aria-hidden="true" />
      <p>{label}</p>
    </div>
  );
}
