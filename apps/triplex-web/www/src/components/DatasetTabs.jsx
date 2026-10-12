/** The dataset the Table and the side panel are showing. Each kind has its own fixed color (see
 * the `--ds-*` tokens in styles.css) so original, synthetic and sketched information can
 * always be told apart, here and on the panel tabs. */
const DATASETS = [
  { id: "original", label: "Original", tier: 1 },
  { id: "synthetic", label: "Synthetic", tier: 2 },
  { id: "sketched", label: "Sketched", tier: 3 },
];

/**
 * @param {{ active: string, onSelect: (id: string) => void, hasSynthetic: boolean,
 *   hasSketched: boolean }} props
 */
export default function DatasetTabs({ active, onSelect, hasSynthetic, hasSketched }) {
  const disabledReason = {
    synthetic: hasSynthetic ? null : "Generate synthetic data first (Tier 2 card)",
    sketched: hasSketched ? null : "Sketch data first (Tier 3 card)",
  };
  return (
    <div className="dataset-tabs" role="tablist" aria-label="Dataset">
      {DATASETS.map((d) => (
        <button
          key={d.id}
          type="button"
          role="tab"
          aria-selected={active === d.id}
          className={`dataset-tab${active === d.id ? " active" : ""}`}
          data-kind={d.id}
          disabled={!!disabledReason[d.id]}
          title={disabledReason[d.id] ?? undefined}
          onClick={() => onSelect(d.id)}
        >
          {d.label}
        </button>
      ))}
    </div>
  );
}
