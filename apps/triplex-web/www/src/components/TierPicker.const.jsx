// Labels from the design notes —
// UI-only; the engine and CLI keep saying Tier 1/2/3 everywhere else. Only
// Tier 1 is interactive in v1; 2 and 3 are shown disabled so the eventual
// three-tier structure is legible from day one. Reversibility text and the
// lock icon's strength come from the design notes's
// "Anonymizer tier detail" table.
export const TIERS = [
  {
    id: 1,
    label: "Basic Anonymization",
    caption: "Uses existing data for masking private data columns",
    reversibility: "100% reversible — mapping key",
    lockStrength: 1,
  },
  {
    id: 2,
    label: "Synthetic Data Generation",
    caption:
      "Uses existing data to generate synthetic data based on (advanced) privacy-preserving algorithms",
    reversibility: "Irreversible — synthetic",
    lockStrength: 2,
  },
  {
    id: 3,
    label: "Pattern-persistent Data Generation",
    caption: "Advanced and pattern-persistent data anonymization",
    reversibility: "Irreversible — synthetic",
    lockStrength: 3,
  },
];
