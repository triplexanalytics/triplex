import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { DataEditor, GridCellKind, drawTextCell } from "@glideapps/glide-data-grid";
import "@glideapps/glide-data-grid/dist/index.css";
import ColumnMenu from "./ColumnMenu";
import { menuIconLocalRect } from "../utils";
import { useTableHeaderCanvas } from "../hooks/useTableHeaderCanvas";
import { HEADER_PADDING, NAME_ROW_HEIGHT, HEADER_HEIGHT, MENU_ICON_SIZE } from "./TableTab.const";
import { readGlideHeaderTheme } from "../state/palette";
import { formatMissingPercent, primaryTypeOf } from "../state/columnFields";
import { recipeSettingsText } from "../state/recipeFile";

// How many frames an outside menu request waits for the grid to know its header bounds.
const MENU_REQUEST_FRAMES = 30;

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
  onSaltChange,
  onAnonymize,
  anonymizing,
  anonymizeResult,
  onCompareAnonymized,
  onColumnHover,
  highlightColumnNames,
  contract,
  onDeclareAsString,
  filteredRowIndices,
  onClearRowFilter,
  onRemoveVisibleRows,
  onRemoveFilteredOutRows,
  removedRowIndices,
  onRestoreRemovedRows,
  readOnly = false,
  dataset = "original",
  onContentWidth,
  menuRequest = null,
  onMenuRequestDone,
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

  const isCategoricalByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? [])
      map[col.name] = col.distribution?.kind === "categorical";
    return map;
  }, [profile]);

  const isLikelyIdentifierByName = useMemo(() => {
    const map = {};
    for (const col of profile?.columns ?? [])
      map[col.name] = col.semantic_type === "likely_identifier";
    return map;
  }, [profile]);

  // Display order (left to right) and per-column width are user-adjustable
  // (drag a header to reorder, drag its right edge to resize) and reset
  // whenever a new file loads — local to this component since nothing else
  // (export, risk scoring, the diversity matrix) cares how the grid happens
  // to be arranged, only about the underlying `sample` data, which this
  // never touches.
  const [columnOrder, setColumnOrder] = useState(() => sample.headers);
  const [columnWidths, setColumnWidths] = useState({}); // name -> width, only entries the user has resized
  useEffect(() => {
    setColumnOrder(sample.headers);
    setColumnWidths({});
  }, [sample]);

  // A removed column is hidden from the grid, not resampled out of it: glide
  // indexes columns by position, so `visibleHeaderIndices[col]` maps a
  // rendered column index back to its real index in `sample`.
  const visibleHeaderIndices = useMemo(() => {
    const indexByName = {};
    sample.headers.forEach((name, i) => (indexByName[name] = i));
    return columnOrder.filter((name) => !removedColumns.has(name)).map((name) => indexByName[name]);
  }, [columnOrder, sample.headers, removedColumns]);

  const DEFAULT_COLUMN_WIDTH = 160;
  const MIN_COLUMN_WIDTH = 60;
  const MAX_COLUMN_WIDTH = 800;
  const columns = useMemo(
    () =>
      visibleHeaderIndices.map((i) => {
        const name = sample.headers[i];
        return { title: name, id: name, width: columnWidths[name] ?? DEFAULT_COLUMN_WIDTH };
      }),
    [visibleHeaderIndices, sample.headers, columnWidths],
  );

  const onColumnResize = useCallback((column, newSize) => {
    setColumnWidths((prev) => ({ ...prev, [column.id]: newSize }));
  }, []);

  // `startIndex`/`endIndex` are positions within `columns` (the *visible*
  // array, already excluding removed columns), not within `columnOrder`
  // (which still lists every column so a removed one keeps its slot, on the
  // off chance a restore feature is ever added). Reorder just the visible
  // subsequence, keeping every removed name pinned at its own slot.
  const onColumnMoved = useCallback(
    (startIndex, endIndex) => {
      setColumnOrder((prev) => {
        const visible = prev.filter((name) => !removedColumns.has(name));
        const [moved] = visible.splice(startIndex, 1);
        visible.splice(endIndex, 0, moved);
        let vi = 0;
        return prev.map((name) => (removedColumns.has(name) ? name : visible[vi++]));
      });
    },
    [removedColumns],
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
  const gridContentWidth = columns.reduce((sum, c) => sum + c.width, 0) + GRID_CHROME_ALLOWANCE;
  // Tell App.jsx, which sizes the dataset tabs' column to match so tabs and table share a left edge.
  useEffect(() => {
    onContentWidth?.(gridContentWidth);
  }, [gridContentWidth, onContentWidth]);

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

  // A matrix cell click (see App.jsx's useRowFilter) narrows this to a
  // specific equivalence class's rows -- filtered *before* sorting, since
  // sort only ever reorders whatever's currently visible, never changes
  // which rows are.
  const visibleRowIndices = useMemo(() => {
    const all = Array.from({ length: sample.rowCount }, (_, i) => i);
    const notRemoved = removedRowIndices?.size ? all.filter((i) => !removedRowIndices.has(i)) : all;
    return filteredRowIndices ? notRemoved.filter((i) => filteredRowIndices.has(i)) : notRemoved;
  }, [sample.rowCount, filteredRowIndices, removedRowIndices]);

  const sortedRowIndices = useMemo(() => {
    const indices = [...visibleRowIndices];
    if (!sort) return indices;
    const colIndex = sample.headers.indexOf(sort.column);
    if (colIndex === -1) return indices;
    // Intl.Collator's numeric mode sorts "2" before "10" (not lexically)
    // and falls back to ordinary text comparison for non-numeric columns —
    // one comparator that works reasonably for every column type without
    // needing to branch on inferred_type here.
    const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
    const sign = sort.direction === "asc" ? 1 : -1;
    indices.sort(
      (a, b) => sign * collator.compare(sample.getCell(a, colIndex), sample.getCell(b, colIndex)),
    );
    return indices;
  }, [sample, sort, visibleRowIndices]);

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
  // own canvas — it has no way to know a header needs repainting (a sort
  // arrow appearing) purely because `drawHeader`'s closure captured new
  // values. Tried `updateCells` ("damage", Glide's imperative repaint API)
  // with row -1 (Glide's own internal sentinel for a column header, per its
  // source) first, since that would be more surgical than a remount — it
  // didn't actually repaint the header in practice, only body cells.
  // Remounting the grid is the one approach confirmed to work;
  // resetting scroll-to-top is also just normal, expected behavior right
  // after a sort anyway, not a regression.
  const [gridKey, setGridKey] = useState(0);
  useEffect(() => {
    setGridKey((k) => k + 1);
  }, [sort]);
  useEffect(() => {
    setGridKey((k) => k + 1);
  }, [visibleHeaderIndices.length]);

  const [menu, setMenu] = useState(null); // { name, x, y, focusSalt? } | null

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
      if (withinIcon && !readOnly) {
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
    [columns, readOnly],
  );

  const { drawHeader, onItemHovered, hoveredHeader, hoveredBin } = useTableHeaderCanvas({
    columns,
    sort,
    typeByName,
    missingByName,
    isCategoricalByName,
    distributionByName,
    onColumnHover,
    hideMenuIcon: readOnly,
  });

  const glideTheme = readGlideHeaderTheme(dataset);

  // A ColumnCard- or RiskMatrix-originated highlight (AnalysisOverlay's
  // Columns/Analysis tabs) can't repaint Glide's header canvas the way a
  // table-originated one can (see useTableHeaderCanvas.js's onItemHovered
  // — that direction just rides Glide's own hover-driven repaint). Instead
  // of fighting the canvas, this draws a plain DOM rectangle per
  // highlighted column on top of its header cell, positioned from
  // DataEditorRef.getBounds(col, -1) -- "-1" is Glide's own row sentinel
  // for a column header, confirmed in its source (data-grid.js: "row: -1
  // === columnHeader"). getBounds returns viewport coordinates (it bakes
  // in the canvas's own getBoundingClientRect()), converted here to be
  // relative to `wrapRef` (`.table-grid-wrap`, which clips it via
  // `overflow: hidden` so a column scrolled out of view doesn't leave the
  // highlight floating over unrelated UI). `pointer-events: none` so the
  // overlay never steals a click/drag/hover meant for the grid underneath
  // it. `highlightColumnNames` can name more than one column at once — a
  // RiskMatrix cell hover pairs a row column with a (possibly different)
  // candidate-sensitive column.
  //
  // ponytail: recomputed only when the highlighted columns or the columns
  // array changes, not on scroll (no onVisibleRegionChanged wiring) --
  // while a ColumnCard/RiskMatrix is hovered the pointer is over the
  // overlay panel, not the grid, so there's no direct way to scroll the
  // table at the same time in practice. Add a scroll recompute if that
  // stops holding.
  const dataEditorRef = useRef(null);
  const wrapRef = useRef(null);
  const [highlightRects, setHighlightRects] = useState([]);
  useEffect(() => {
    const grid = dataEditorRef.current;
    const wrap = wrapRef.current;
    if (!grid || !wrap || !highlightColumnNames?.length) {
      setHighlightRects([]);
      return;
    }
    const wrapBounds = wrap.getBoundingClientRect();
    const rects = highlightColumnNames
      .map((name) => {
        const colIndex = columns.findIndex((c) => c.id === name);
        const bounds = colIndex === -1 ? undefined : grid.getBounds(colIndex, -1);
        if (!bounds) return null;
        return {
          key: name,
          left: bounds.x - wrapBounds.left,
          top: bounds.y - wrapBounds.top,
          width: bounds.width,
          height: bounds.height,
        };
      })
      .filter(Boolean);
    setHighlightRects(rects);
  }, [highlightColumnNames, columns]);

  // A request from outside (an opened recipe with a keyed column) to open one column's menu
  // with the cursor in the salt field. Opened at the column's header icon, or at the table's
  // corner when the column is out of view; the menu keeps itself inside the window either way.
  // Reported done right away, so a later remount of the table does not open it again. Placed
  // once the grid can say where the header is, tried for a few frames: the same recipe can hide
  // dropped columns, which remounts the grid (see gridKey), and a remounted grid knows its
  // header bounds only after it has laid itself out.
  const columnsRef = useRef(columns);
  columnsRef.current = columns;
  useEffect(() => {
    if (!menuRequest) return;
    onMenuRequestDone?.();
    if (readOnly) return;
    const { name } = menuRequest;
    const place = (framesLeft) => {
      const colIndex = columnsRef.current.findIndex((c) => c.id === name);
      const bounds = colIndex === -1 ? undefined : dataEditorRef.current?.getBounds(colIndex, -1);
      if (bounds) {
        const icon = menuIconLocalRect(bounds.width, ICON_LAYOUT);
        setMenu({ name, x: bounds.x + icon.x, y: bounds.y + icon.y + icon.size, focusSalt: true });
      } else if (framesLeft > 0) {
        requestAnimationFrame(() => place(framesLeft - 1));
      } else {
        const wrap = wrapRef.current?.getBoundingClientRect();
        setMenu({ name, x: (wrap?.left ?? 0) + 8, y: (wrap?.top ?? 0) + 8, focusSalt: true });
      }
    };
    requestAnimationFrame(() => place(MENU_REQUEST_FRAMES));
    // Only a new request opens the menu, not a change of columns or of the callback.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [menuRequest]);

  return (
    <div
      ref={wrapRef}
      className="table-grid-wrap"
      style={{ position: "relative", height: "100%", maxWidth: gridContentWidth, margin: "0 auto" }}
    >
      <DataEditor
        ref={dataEditorRef}
        key={gridKey}
        columns={columns}
        rows={sortedRowIndices.length}
        // @ts-expect-error -- glide-data-grid's GridCell/CustomRenderer
        // types are discriminated unions keyed on a type-predicate
        // (`cell is X`), which plain JS has no JSDoc syntax to express;
        // these two callbacks are correct at runtime (kind/data always
        // agree), just outside what checkJs can verify.
        getCellContent={getCellContent}
        // @ts-expect-error -- see getCellContent above.
        customRenderers={customRenderers}
        onHeaderClicked={onHeaderClicked}
        drawHeader={drawHeader}
        onItemHovered={onItemHovered}
        theme={glideTheme}
        headerHeight={HEADER_HEIGHT}
        onColumnResize={onColumnResize}
        onColumnMoved={onColumnMoved}
        minColumnWidth={MIN_COLUMN_WIDTH}
        maxColumnWidth={MAX_COLUMN_WIDTH}
        smoothScrollX
        smoothScrollY
      />
      {highlightRects.map((rect) => (
        <div
          key={rect.key}
          className="table-header-highlight"
          style={{
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
          }}
        />
      ))}
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
          currentSettings={recipeSettingsText(recipe.columns.find((c) => c.name === menu.name))}
          currentSalt={salt}
          focusSalt={!!menu.focusSalt}
          onPickMethod={(method) => onPickMethod(menu.name, method)}
          onSaltChange={onSaltChange}
          onAnonymize={onAnonymize}
          anonymizing={anonymizing}
          onClose={() => setMenu(null)}
          isLikelyIdentifier={isLikelyIdentifierByName[menu.name]}
          declaredAsString={contract?.columns.find((c) => c.name === menu.name)?.type === "string"}
          onDeclareAsString={onDeclareAsString}
        />
      )}
      {anonymizeResult && (
        <div className="banner banner-success anonymized-notice">
          Anonymized {JSON.parse(anonymizeResult.report_json).rows_processed} rows.
          {onCompareAnonymized && (
            <button type="button" onClick={onCompareAnonymized}>
              Compare with the original
            </button>
          )}
        </div>
      )}
      {(filteredRowIndices || removedRowIndices?.size > 0) && (
        <div className="table-status-banners">
          {filteredRowIndices && (
            <div className="banner banner-warning">
              Filtered to {sortedRowIndices.length} of {sample.rowCount} rows.
              <button type="button" onClick={onClearRowFilter}>
                Clear
              </button>
              {onRemoveVisibleRows && (
                <button
                  type="button"
                  title="Remove these (shown) rows from the anonymize/download output"
                  onClick={onRemoveVisibleRows}
                >
                  Remove visible rows
                </button>
              )}
              {onRemoveFilteredOutRows && (
                <button
                  type="button"
                  title="Remove every row NOT shown from the anonymize/download output — keep only these"
                  onClick={onRemoveFilteredOutRows}
                >
                  Remove filtered rows
                </button>
              )}
            </div>
          )}
          {removedRowIndices?.size > 0 && (
            <div className="banner banner-warning">
              {removedRowIndices.size} row{removedRowIndices.size === 1 ? "" : "s"} removed from
              output.
              <button type="button" onClick={onRestoreRemovedRows}>
                Restore
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
