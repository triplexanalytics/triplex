/** A single padlock glyph shared by all three tiers — only its color
 * changes (via the `strength-N` class, see styles.css), one base hue
 * (`--accent`) at increasing intensity, rather than three different icons
 * or three unrelated colors. */
export default function LockIcon({ strength = 1 }) {
  return (
    <svg
      className={`lock-icon strength-${strength}`}
      width="15"
      height="15"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      <rect x="3" y="7" width="10" height="7" rx="1.5" fill="currentColor" />
      <path d="M5 7V5a3 3 0 0 1 6 0v2" stroke="currentColor" strokeWidth="1.6" fill="none" />
    </svg>
  );
}
