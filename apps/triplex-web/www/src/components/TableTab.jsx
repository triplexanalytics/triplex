import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { DataEditor, GridCellKind, drawTextCell } from "@glideapps/glide-data-grid";
import "@glideapps/glide-data-grid/dist/index.css";
import ColumnMenu from "./ColumnMenu";
import { menuIconLocalRect } from "../utils";
import { useTableHeaderCanvas } from "../hooks/useTableHeaderCanvas";
import { HEADER_PADDING, NAME_ROW_HEIGHT, HEADER_HEIGHT, MENU_ICON_SIZE } from "./TableTab.const";
import { readGlideHeaderTheme } from "../state/palette";
import { formatMissingPercent, primaryTypeOf } from "../state/columnFields";

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
  contract,
  onDeclareAsString,
}) {
  const distributionByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? []) map[col.name] = col.distribution;
    return map;
  }, [profile]);

  const typeByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? []) map[col.name] = primaryTypeOf(col);
    return map;
  }, [profile]);

  // ColumnCard's "Missing" stat, same source (columnFields.js) so the
  // header's reduced field set stays exactly in sync with the card's.
  const missingByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? []) map[col.name] = formatMissingPercent(col);
    return map;
  }, [profile]);

  // Separate from typeByName: the risk bar/distinct-count row draws for
  // whatever column useColumnRisk actually scores (integer, generalized
  // into a bucket, or categorical/string, scored on exact values — see
  // the design notes), which needs the bare primitive/distribution kind
  // regardless of whatever semantic type is also displayed.
  const isRiskableByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? []) {
      map[col.name] = col.inferred_type === "integer" || col.distribution?.kind === "categorical";
    }
    return map;
  }, [profile]);

  const isCategoricalByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? []) map[col.name] = col.distribution?.kind === "categorical";
    return map;
  }, [profile]);

  const isLikelyIdentifierByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? []) map[col.name] = col.semantic_type === "likely_identifier";
    return map;
  }, [profile]);

  // A removed column is hidden from the grid, not resampled out of it: glide
  // indexes columns by position, so `visibleHeaderIndices[col]` maps a
  // rendered column index back to its real index in `sample`.
  const visibleHeaderIndices = useMemo(
    () => sample.headers.map((_, i) => i).filter((i) => !removedColumns.has(sample.headers[i])),
    [sample.headers, removedColumns],
  );

  const COLUMN_WIDTH = 160;
  const columns = useMemo(
    () => visibleHeaderIndices.map((i) => ({ title: sample.headers[i], id: sample.headers[i], width: COLUMN_WIDTH })),
    [visibleHeaderIndices, sample.headers],
  );
  // Glide Data Grid fills its container's width (no `width` prop passed to
  // DataEditor below) rather than sizing to its own content, so a narrow
  // file's columns would otherwise sit pinned to the left edge with the
  // rest of a wide viewport left blank. Capping the wrapper at the grid's
  // actual content width and centering it fixes that; a file with enough
  // columns to exceed the viewport still fills it edge-to-edge and scrolls
  // normally, since `maxWidth` only ever constrains, never forces overflow.
  //
  // GRID_CHROME_ALLOWANCE: when there are enough rows to need a vertical
  // scrollbar, Glide draws its own (not the OS's) in a fixed-width gutter
  // that eats into the horizontal canvas — without this slack, capping the
  // wrapper at exactly the columns' total width leaves the scroller's
  // virtual content a few px wider than its now-cramped drawable area,
  // which shows an unwanted horizontal scrollbar too (confirmed via a
  // DOM width dump: `.dvn-scroller` came out 8px wider than its own
  // `clientWidth` with no allowance). Empirical, not measured from Glide's
  // own layout constants — generous enough to cover it.
  const GRID_CHROME_ALLOWANCE = 16;
  const gridContentWidth = columns.length * COLUMN_WIDTH + GRID_CHROME_ALLOWANCE;

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
    missingByName,
    isRiskableByName,
    isCategoricalByName,
    distributionByName,
    riskByName,
    riskLoading,
  });

  // Re-read every render rather than once at module load, so a light/dark
  // toggle picks up the new --text/--text-muted/... values — though nothing
  // here forces a re-render on an OS theme change the way RiskMatrix.jsx's
  // colorTick does; the next unrelated re-render (sort, data reload, ...)
  // is what actually repaints the header. ponytail: fine for now, add a
  // matching prefers-color-scheme listener if a bare toggle-with-no-other-
  // interaction turns out to matter in practice.
  const glideTheme = readGlideHeaderTheme();

  return (
    <div
      className="table-grid-wrap"
      style={{ position: "relative", height: "100%", maxWidth: gridContentWidth, margin: "0 auto" }}
    >
      <DataEditor
        key={gridKey}
        columns={columns}
        rows={sample.rowCount}
        getCellContent={getCellContent}
        customRenderers={customRenderers}
        onHeaderClicked={onHeaderClicked}
        drawHeader={drawHeader}
        onItemHovered={onItemHovered}
        theme={glideTheme}
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
          isLikelyIdentifier={isLikelyIdentifierByName[menu.name]}
          declaredAsString={contract?.columns.find((c) => c.name === menu.name)?.type === "string"}
          onDeclareAsString={onDeclareAsString}
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
