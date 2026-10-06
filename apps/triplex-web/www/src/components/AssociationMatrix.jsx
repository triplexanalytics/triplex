import { useCallback, useMemo } from "react";
import RiskMatrix from "./RiskMatrix";
import { MEASURE_LABEL } from "../state/association";

const INFO =
  "How strongly two columns move together. Numbers use Pearson r (the sign is in the tooltip), " +
  "categories use Cramér's V, and a category against a number uses the correlation ratio. " +
  "Color is the strength from 0 to 1; the scales differ by measure, so compare cells of the same " +
  "measure. Identifier columns are left out. A measurement of this table, not a privacy guarantee.";

const fmtSigned = (x) => (x >= 0 ? `+${x.toFixed(2)}` : x.toFixed(2));

function hoverProps(hoverNames, onHoverNamesChange) {
  return {
    highlightRowName: hoverNames?.rowName,
    highlightColName: hoverNames?.colName,
    onHoverChange: onHoverNamesChange,
  };
}

/** One dataset's association heatmap (every eligible column against every other). */
export function AssociationMatrix({ association, names, title, hoverNames, onHoverNamesChange }) {
  // `names` (optional) is a shared axis wider than this dataset's own columns, so a comparison
  // draws both datasets on the same grid; a column this dataset lacks is an empty row and column.
  const axis = useMemo(() => names ?? association?.names ?? [], [names, association]);
  const cellAt = useCallback(
    (i, j) => {
      const ri = association?.names.indexOf(axis[i]);
      const ci = association?.names.indexOf(axis[j]);
      const cell = ri < 0 || ci < 0 ? null : association?.cells[ri]?.[ci];
      if (!cell) return null;
      const shown = cell.signed ?? cell.value;
      return {
        risk: cell.value,
        label: cell.signed === null ? cell.value.toFixed(2) : fmtSigned(shown),
        tooltip: {
          title: `${axis[i]} × ${axis[j]}`,
          detail: `${MEASURE_LABEL[cell.measure]} ${cell.signed === null ? cell.value.toFixed(2) : fmtSigned(shown)}`,
        },
      };
    },
    [association, axis],
  );
  if (!association) return null;
  if (association.names.length < 2) {
    return <p className="synth-hint">Needs at least two number or category columns.</p>;
  }
  return (
    <RiskMatrix
      title={title}
      rows={axis}
      cols={axis}
      infoText={INFO}
      cellAt={cellAt}
      legendLowLabel="none"
      legendHighLabel="strong"
      onCellClick={undefined}
      {...hoverProps(hoverNames, onHoverNamesChange)}
    />
  );
}

/** Right minus left, for the columns both datasets have. Color is the size of the change. */
export function AssociationDiffMatrix({
  left,
  right,
  names,
  title,
  hoverNames,
  onHoverNamesChange,
}) {
  const shared = useMemo(() => {
    if (!left || !right) return [];
    return names ?? left.names.filter((n) => right.names.includes(n));
  }, [left, right, names]);
  const cellAt = useCallback(
    (i, j) => {
      const li = left.names.indexOf(shared[i]);
      const lj = left.names.indexOf(shared[j]);
      const ri = right.names.indexOf(shared[i]);
      const rj = right.names.indexOf(shared[j]);
      if (li < 0 || lj < 0 || ri < 0 || rj < 0) return null;
      const a = left.cells[li]?.[lj];
      const b = right.cells[ri]?.[rj];
      if (!a || !b || a.measure !== b.measure) return null;
      const delta = (b.signed ?? b.value) - (a.signed ?? a.value);
      return {
        risk: Math.min(1, Math.abs(delta)),
        label: fmtSigned(delta),
        tooltip: {
          title: `${shared[i]} × ${shared[j]}`,
          detail: `${MEASURE_LABEL[a.measure]}: ${(a.signed ?? a.value).toFixed(2)} → ${(b.signed ?? b.value).toFixed(2)} (${fmtSigned(delta)})`,
        },
      };
    },
    [left, right, shared],
  );
  if (shared.length < 2) return null;
  return (
    <RiskMatrix
      title={title}
      rows={shared}
      cols={shared}
      infoText="The change in association between the two datasets (second minus first), for the columns both have. Darker means the relationship changed more."
      cellAt={cellAt}
      legendLowLabel="no change"
      legendHighLabel="large change"
      onCellClick={undefined}
      {...hoverProps(hoverNames, onHoverNamesChange)}
    />
  );
}
