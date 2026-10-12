// Labels are UI-only; the engine and CLI keep saying Tier 1/2/3 everywhere else. Tiers
// 2 and 3 need a loaded file. Reversibility text and the
// lock icon's strength follow each tier's guarantees.
export const TIERS = [
  {
    id: 1,
    label: "Basic Anonymization",
    caption: "Uses existing data for masking private data columns",
    reversibility: "100% reversible (mapping key)",
    lockStrength: 1,
  },
  {
    id: 2,
    label: "Synthetic Data Generation",
    caption:
      "Uses existing data to generate synthetic data based on (advanced) privacy-preserving algorithms",
    reversibility: "Irreversible (synthetic)",
    lockStrength: 2,
  },
  {
    id: 3,
    label: "Pattern-persistent Data Generation",
    caption:
      "Sketch your data as classes on parallel coordinates, steer the sketch, generate from it",
    reversibility: "Irreversible (synthetic)",
    lockStrength: 3,
  },
];
