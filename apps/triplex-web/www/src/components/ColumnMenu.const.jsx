// Placeholder options only — no backend support exists yet (no imputation
// logic anywhere in `triplex-schemacast`/`triplex-anonymizer`). Shown
// disabled so the eventual per-column decision set is legible, the same
// pattern already used for the disabled Tier 2/3 cards in the navbar.
export const IMPUTATION_OPTIONS = ["Mean", "Median", "Mode", "Constant", "Drop rows"];

// One-line description for the info tooltip next to each method radio.
// Kept here (not inline in ColumnMenu.jsx) so every piece of copy this
// component displays lives in one importable place — the natural seam for
// an eventual i18n pass, which would replace these string values with
// lookups keyed by the same method names, not restructure the component.
// `methods` itself comes from `list_anonymize_methods()` (backend stays the
// source of truth for *which* methods exist); this map only supplies the
// human-readable blurb and is missing-safe for a method it doesn't know yet.
export const METHOD_INFO = {
  token_hmac:
    "Deterministic HMAC token: the same input always produces the same pseudonym under a given salt, so joins across columns/files still work. Not reversible without the salt.",
};
