const TABS = [
  { id: "table", label: "Table" },
  { id: "column", label: "Dataset Anonymity Analysis" },
];

export default function Tabs({ active, onChange }) {
  return (
    <div className="tabs">
      {TABS.map((tab) => (
        <button
          key={tab.id}
          type="button"
          className={`tab-button${active === tab.id ? " active" : ""}`}
          onClick={() => onChange(tab.id)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
