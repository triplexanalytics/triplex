import { Hash, KeyRound, Layers, EyeOff, FileX, Wrench } from "lucide-react";

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
    "Deterministic HMAC (Hash-based Message Authentication Code) token: the same input always produces the same pseudonym under a given salt, so joins across columns/files still work. Not reversible without the salt.",
  fpe: "Format-Preserving Encryption (FPE): a keyed, per-position substitution that keeps the value's length and character class intact (e.g. a 16-digit card number stays a 16-digit number). Deterministic under a given salt; not reversible without it.",
  bucket:
    'Numeric generalization, also known as k-anonymity bucketing: replaces a value with the numeric range it falls in, e.g. 23 → "20-29". Salt is unused — every run buckets the same way.',
  partial_mask:
    "Keeps a fixed prefix and suffix of the value and replaces everything between with *, e.g. a card number showing only its last 4 digits. Salt is unused.",
  drop: "Excludes this column from the anonymized output entirely -- unlike the other methods, nothing about its values is masked and kept; the column just doesn't appear in the downloaded file at all. Salt is unused.",
};

// Icon + category per method, for the card-grid picker — a method not
// listed here (new backend variant the UI hasn't caught up to yet) falls
// back to a generic tool glyph rather than breaking the grid. `label`
// overrides the card's own display name (normally just the raw method
// string, e.g. "token_hmac") for a method where that raw name wouldn't
// read as an action — every other method still shows its raw name.
export const METHOD_META = {
  token_hmac: { icon: Hash, category: "pseudonymize" },
  fpe: { icon: KeyRound, category: "encrypt" },
  bucket: { icon: Layers, category: "generalize" },
  partial_mask: { icon: EyeOff, category: "mask" },
  drop: { icon: FileX, category: "drop", label: "Delete from output" },
};
export const METHOD_META_FALLBACK = { icon: Wrench, category: "default" };
