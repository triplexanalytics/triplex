import { useCallback, useMemo, useRef, useState } from "react";
import { DataEditor, GridCellKind } from "@glideapps/glide-data-grid";
import "@glideapps/glide-data-grid/dist/index.css";
import ColumnMenu from "./ColumnMenu";
import { barsFor, binTooltip } from "../state/binTooltip";
import { truncateToWidth, menuIconLocalRect, drawMenuIcon } from "../utils";
import {
  HEADER_PADDING,
  NAME_ROW_HEIGHT,
  TYPE_ROW_HEIGHT,
  SPARK_HEIGHT,
  SPARK_TOP,
  HEADER_HEIGHT,
  MENU_ICON_SIZE,
} from "./TableTab.const";

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
  onSaltChange,
  onAnonymize,
  anonymizeResult,
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
      // header than the bare primitive type it's layered on top of.
      map[col.name] = col.semantic_type ?? col.inferred_type;
    }
    return map;
  }, [profile]);

  const columns = useMemo(
    () => sample.headers.map((name) => ({ title: name, id: name, width: 160 })),
    [sample.headers],
  );

  const getCellContent = useCallback(
    ([col, row]) => {
      const value = sample.getCell(row, col);
      return {
        kind: GridCellKind.Text,
        data: value,
        displayData: value,
        allowOverlay: false,
      };
    },
    [sample],
  );

  // Whether the last draw pass had to truncate each column's name — read by
  // the hover tooltip below to decide whether there's anything worth
  // showing. A plain ref, not state: it's written every paint and read only
  // on hover, so it doesn't need to trigger a re-render itself.
  const truncatedRef = useRef({});
  const [hoveredHeader, setHoveredHeader] = useState(null);
  const [hoveredBin, setHoveredBin] = useState(null);
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
      if (!withinIcon) return;
      setMenu({
        name: column.id,
        x: event.bounds.x + icon.x,
        y: event.bounds.y + icon.y + icon.size,
      });
    },
    [columns],
  );

  // Header cells are canvas-rendered by the grid itself (not DOM), so the
  // name/type/sparkline/menu-icon layout has to be drawn directly into the
  // grid's own header-drawing pass rather than mounted as React nodes.
  // Returning `true` tells Glide we drew the whole cell ourselves —
  // including the title, laid out here on its own line above the type — so
  // it shouldn't also draw its default single-line title on top.
  const drawHeader = useCallback(
    (args) => {
      const { ctx, rect, column, theme } = args;
      const innerX = rect.x + HEADER_PADDING;
      const icon = menuIconLocalRect(rect.width, ICON_LAYOUT);
      const nameMaxWidth = icon.x - HEADER_PADDING - 4;
      const fontFamily = theme?.fontFamily ?? "sans-serif";

      ctx.textBaseline = "middle";

      ctx.font = `600 13px ${fontFamily}`;
      ctx.fillStyle = theme?.textHeader ?? "#1a1d24";
      const { text: nameText, truncated } = truncateToWidth(ctx, column.title, nameMaxWidth);
      ctx.fillText(nameText, innerX, rect.y + NAME_ROW_HEIGHT / 2 + 2);
      truncatedRef.current[column.id] = truncated;

      drawMenuIcon(ctx, rect.x + icon.x, rect.y + icon.y, icon.size, theme);

      const colType = typeByName[column.id];
      if (colType) {
        ctx.font = `italic 11px ${fontFamily}`;
        ctx.fillStyle = theme?.textLight ?? "#8890a0";
        ctx.fillText(colType, innerX, rect.y + NAME_ROW_HEIGHT + TYPE_ROW_HEIGHT / 2);
      }

      const bars = barsFor(distributionByName[column.id]);
      const max = Math.max(0, ...bars);
      if (max > 0) {
        const barAreaTop = rect.y + rect.height - SPARK_HEIGHT - 4;
        const barWidth = rect.width / bars.length;
        ctx.fillStyle = "#818cf8";
        bars.forEach((count, i) => {
          const h = (count / max) * SPARK_HEIGHT;
          ctx.fillRect(
            rect.x + i * barWidth,
            barAreaTop + (SPARK_HEIGHT - h),
            Math.max(barWidth - 1, 1),
            h,
          );
        });
      }

      return true;
    },
    [distributionByName, typeByName],
  );

  // Same header hover callback drives two mutually-exclusive tooltips: the
  // truncated-name one (top two rows) and the histogram-bin one (bottom
  // sparkline row) — whichever region the cursor is actually over.
  const onItemHovered = useCallback(
    (args) => {
      if (args.kind !== "header") {
        setHoveredHeader(null);
        setHoveredBin(null);
        return;
      }
      const column = columns[args.location[0]];
      if (!column) {
        setHoveredHeader(null);
        setHoveredBin(null);
        return;
      }

      const localX = args.localEventX ?? 0;
      const localY = args.localEventY ?? 0;
      if (localY >= SPARK_TOP) {
        const distribution = distributionByName[column.id];
        const bars = barsFor(distribution);
        if (bars.length > 0) {
          const barWidth = args.bounds.width / bars.length;
          const index = Math.min(Math.max(Math.floor(localX / barWidth), 0), bars.length - 1);
          const info = binTooltip(distribution, index);
          if (info) {
            setHoveredHeader(null);
            // Anchored to the cursor's own position (bounds + local offset
            // reconstructs the exact viewport coordinate), not to a
            // re-derived bar center — immune to any bar-geometry mismatch
            // between this calculation and what `drawHeader` actually drew.
            setHoveredBin({
              ...info,
              x: args.bounds.x + localX,
              y: args.bounds.y + localY,
            });
            return;
          }
        }
      }
      setHoveredBin(null);

      if (truncatedRef.current[column.id]) {
        setHoveredHeader({
          name: column.title,
          type: typeByName[column.id],
          x: args.bounds.x,
          y: args.bounds.y + args.bounds.height,
        });
      } else {
        setHoveredHeader(null);
      }
    },
    [columns, typeByName, distributionByName],
  );

  return (
    <div style={{ position: "relative", height: "100%" }}>
      <DataEditor
        columns={columns}
        rows={sample.rowCount}
        getCellContent={getCellContent}
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
          {hoveredHeader.type && <em> — {hoveredHeader.type}</em>}
        </div>
      )}
      {hoveredBin && (
        <div
          className="bin-tooltip"
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
