/** The dataset the Table and the side panel are showing: the original and one tab per generator
 * extension. Each kind has its own fixed color (see the `--ds-*` tokens in styles.css) so
 * original, synthetic and sketched information can always be told apart, here and on the panel
 * tabs.
 *
 * @param {{ active: string, onSelect: (id: string) => void,
 *   datasets: { id: string, label: string, disabledReason: string | null }[] }} props
 */
export default function DatasetTabs({ active, onSelect, datasets }) {
  return (
    <div className="dataset-tabs" role="tablist" aria-label="Dataset">
      {datasets.map((d) => (
        <button
          key={d.id}
          type="button"
          role="tab"
          aria-selected={active === d.id}
          className={`dataset-tab${active === d.id ? " active" : ""}`}
          data-kind={d.id}
          disabled={!!d.disabledReason}
          title={d.disabledReason ?? undefined}
          onClick={() => onSelect(d.id)}
        >
          {d.label}
        </button>
      ))}
    </div>
  );
}
