import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { DataEditor, GridCellKind, drawTextCell } from "@glideapps/glide-data-grid";
import "@glideapps/glide-data-grid/dist/index.css";
import ColumnMenu from "./ColumnMenu";
import { menuIconLocalRect } from "../utils";
import { useTableHeaderCanvas } from "../hooks/useTableHeaderCanvas";
import { HEADER_PADDING, NAME_ROW_HEIGHT, HEADER_HEIGHT, MENU_ICON_SIZE } from "./TableTab.const";

const ICON_LAYOUT = {
  headerPadding: HEADER_PADDING,
  menuIconSize: MENU_ICON_SIZE,
  nameRowHeight: NAME_ROW_HEIGHT,
};

export default function TableTab({
  sample,
  profile,
  methods,
  recipe,
  salt,
  onPickMethod,
  removedColumns,
  onRemoveColumn,
  onSaltChange,
  onAnonymize,
  anonymizing,
  anonymizeResult,
  riskByName,
  riskLoading,
}) {
  const distributionByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? []) map[col.name] = col.distribution;
    return map;
  }, [profile]);

  const typeByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? []) {
      // A detected semantic type (iban, address, ...) is more useful in the
      // header than the bare primitive type it's layered on top of — also
      // the lookup key for both the icon (TYPE_ICON_NODES) and the label.
      map[col.name] = col.semantic_type ?? col.inferred_type;
    }
    return map;
  }, [profile]);

  // Separate from typeByName: the risk bar is k-anonymity-only (integer
  // columns), which needs the bare primitive type regardless of whatever
  // semantic type is also displayed.
  const isIntegerByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? []) map[col.name] = col.inferred_type === "integer";
    return map;
  }, [profile]);

  // A removed column is hidden from the grid, not resampled out of it: glide
  // indexes columns by position, so `visibleHeaderIndices[col]` maps a
  // rendered column index back to its real index in `sample`.
  const visibleHeaderIndices = useMemo(
    () => sample.headers.map((_, i) => i).filter((i) => !removedColumns.has(sample.headers[i])),
    [sample.headers, removedColumns],
  );

  const columns = useMemo(
    () => visibleHeaderIndices.map((i) => ({ title: sample.headers[i], id: sample.headers[i], width: 160 })),
    [visibleHeaderIndices, sample.headers],
  );

  // A column with a method picked, once anonymize has actually run, renders
  // its cells blurred (native canvas ctx.filter, no library) — hovering a
  // cell fades the blur out via Glide's own per-cell hoverAmount animation.
  // The value drawn is always the original `sample` value: no masked data
  // is plumbed into the grid, so "hover reveals" is literally "stop hiding
  // what was already there" rather than a second data source to keep in sync.
  const blurredColumns = useMemo(() => {
    if (!anonymizeResult) return new Set();
    return new Set(recipe.columns.map((c) => c.name));
  }, [anonymizeResult, recipe]);

  const customRenderers = useMemo(
    () => [
      {
        kind: GridCellKind.Custom,
        isMatch: (c) => c.data?.kind === "blurred-value",
        needsHover: true,
        needsHoverPosition: false,
        draw: (args) => {
          const { ctx, cell, hoverAmount } = args;
          const blurPx = 2.5 * (1 - hoverAmount);
          ctx.save();
          if (blurPx > 0.1) ctx.filter = `blur(${blurPx}px)`;
          drawTextCell(args, cell.data.value, "left");
          ctx.restore();
        },
        onPaste: () => undefined,
      },
    ],
    [],
  );

  // Glide has no built-in sort — clicking a column name (see onHeaderClicked
  // below) cycles asc -> desc -> none, and this reorders row *indices* only.
  // sample itself stays untouched: every other consumer of the raw sample
  // (risk scoring, the diversity matrix, CSV export) keys off original row
  // order and must keep doing so regardless of how the table happens to be
  // displayed.
  const [sort, setSort] = useState(null); // { column, direction: "asc" | "desc" } | null

  const sortedRowIndices = useMemo(() => {
    const indices = Array.from({ length: sample.rowCount }, (_, i) => i);
    if (!sort) return indices;
    const colIndex = sample.headers.indexOf(sort.column);
    if (colIndex === -1) return indices;
    // Intl.Collator's numeric mode sorts "2" before "10" (not lexically)
    // and falls back to ordinary text comparison for non-numeric columns —
    // one comparator that works reasonably for every column type without
    // needing to branch on inferred_type here.
    const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
    const sign = sort.direction === "asc" ? 1 : -1;
    indices.sort((a, b) => sign * collator.compare(sample.getCell(a, colIndex), sample.getCell(b, colIndex)));
    return indices;
  }, [sample, sort]);

  const getCellContent = useCallback(
    ([col, row]) => {
      const originalRow = sortedRowIndices[row];
      const originalCol = visibleHeaderIndices[col];
      const value = sample.getCell(originalRow, originalCol);
      if (blurredColumns.has(columns[col]?.id)) {
        return {
          kind: GridCellKind.Custom,
          data: { kind: "blurred-value", value },
          copyData: value,
          allowOverlay: false,
        };
      }
      return {
        kind: GridCellKind.Text,
        data: value,
        displayData: value,
        allowOverlay: false,
      };
    },
    [sample, sortedRowIndices, visibleHeaderIndices, blurredColumns, columns],
  );

  // Glide draws each header cell once via `drawHeader` and caches it on its
  // own canvas — it has no way to know a header needs repainting (the risk
  // bar going from "loading" to "done", or a sort arrow appearing) purely
  // because `drawHeader`'s closure captured new values. Tried `updateCells`
  // ("damage", Glide's imperative repaint API) with row -1 (Glide's own
  // internal sentinel for a column header, per its source) first, since
  // that would be more surgical than a remount — it didn't actually
  // repaint the header in practice, only body cells. Remounting the grid
  // is the one approach confirmed to work; resetting scroll-to-top is also
  // just normal, expected behavior right after a sort anyway, not a
  // regression the way it would be for the risk-bar-loading case.
  const [gridKey, setGridKey] = useState(0);
  const wasRiskLoading = useRef(riskLoading);
  useEffect(() => {
    if (wasRiskLoading.current && !riskLoading) {
      setGridKey((k) => k + 1);
    }
    wasRiskLoading.current = riskLoading;
  }, [riskLoading]);
  useEffect(() => {
    setGridKey((k) => k + 1);
  }, [sort]);
  useEffect(() => {
    setGridKey((k) => k + 1);
  }, [visibleHeaderIndices.length]);

  const [menu, setMenu] = useState(null); // { name, x, y } | null

  const onHeaderClicked = useCallback(
    (colIndex, event) => {
      const column = columns[colIndex];
      if (!column) return;
      const icon = menuIconLocalRect(event.bounds.width, ICON_LAYOUT);
      const withinIcon =
        (event.localEventX ?? -1) >= icon.x &&
        event.localEventX <= icon.x + icon.size &&
        (event.localEventY ?? -1) >= icon.y &&
        event.localEventY <= icon.y + icon.size;
      if (withinIcon) {
        setMenu({
          name: column.id,
          x: event.bounds.x + icon.x,
          y: event.bounds.y + icon.y + icon.size,
        });
        return;
      }
      // Clicking the name row (anywhere but the menu icon) cycles this
      // column's sort: none -> asc -> desc -> none. Picking a different
      // column always starts it fresh at asc, not wherever the previous
      // column's cycle left off.
      const localY = event.localEventY ?? -1;
      if (localY >= 0 && localY < NAME_ROW_HEIGHT) {
        setSort((prev) => {
          if (!prev || prev.column !== column.id) return { column: column.id, direction: "asc" };
          if (prev.direction === "asc") return { column: column.id, direction: "desc" };
          return null;
        });
      }
    },
    [columns],
  );

  const { drawHeader, onItemHovered, hoveredHeader, hoveredBin } = useTableHeaderCanvas({
    columns,
    sort,
    typeByName,
    isIntegerByName,
    distributionByName,
    riskByName,
    riskLoading,
  });

  return (
    <div className="table-grid-wrap" style={{ position: "relative", height: "100%" }}>
      <DataEditor
        key={gridKey}
        columns={columns}
        rows={sample.rowCount}
        getCellContent={getCellContent}
        customRenderers={customRenderers}
        onHeaderClicked={onHeaderClicked}
        drawHeader={drawHeader}
        onItemHovered={onItemHovered}
        headerHeight={HEADER_HEIGHT}
        smoothScrollX
        smoothScrollY
      />
      {hoveredHeader && (
        <div
          className="header-tooltip"
          style={{ position: "fixed", left: hoveredHeader.x, top: hoveredHeader.y }}
        >
          <strong>{hoveredHeader.name}</strong>
          {hoveredHeader.type && <em> ({hoveredHeader.type})</em>}
        </div>
      )}
      {hoveredBin && (
        <div
          className={`bin-tooltip${hoveredBin.wrap ? " wrap" : ""}`}
          style={{
            position: "fixed",
            left: hoveredBin.x,
            top: hoveredBin.y,
            transform: "translate(-50%, -100%)",
          }}
        >
          <strong>{hoveredBin.title}</strong>
          <span>{hoveredBin.detail}</span>
        </div>
      )}
      {menu && (
        <ColumnMenu
          columnName={menu.name}
          x={menu.x}
          y={menu.y}
          methods={methods}
          currentMethod={recipe.columns.find((c) => c.name === menu.name)?.method}
          currentSalt={salt}
          onPickMethod={(method) => onPickMethod(menu.name, method)}
          onSaltChange={onSaltChange}
          onAnonymize={onAnonymize}
          anonymizing={anonymizing}
          onRemove={() => {
            onRemoveColumn(menu.name);
            setMenu(null);
          }}
          onClose={() => setMenu(null)}
        />
      )}
      {anonymizeResult && (
        <div className="banner banner-success" style={{ position: "absolute", left: 8, bottom: 8 }}>
          Anonymized {JSON.parse(anonymizeResult.report_json).rows_processed} rows.
        </div>
      )}
    </div>
  );
}
