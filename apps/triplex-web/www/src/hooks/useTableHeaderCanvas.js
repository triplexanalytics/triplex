import { useCallback, useRef, useState } from "react";
import { barsFor, binTooltip } from "../state/binTooltip";
import { truncateToWidth, menuIconLocalRect, drawMenuIcon, drawSortArrow } from "../utils";
import { TYPE_ICON_NODES, drawIconNode } from "../state/typeIconNodes";
import { readRiskColorStops, reciprocalRisk } from "../state/riskColor";
import { kAnonymityExplanation } from "../state/riskExplanations";
import {
  HEADER_PADDING,
  NAME_ROW_HEIGHT,
  TYPE_ROW_HEIGHT,
  TYPE_ICON_SIZE,
  RISK_ROW_HEIGHT,
  RISK_BAR_HEIGHT,
  SPARK_HEIGHT,
  SPARK_TOP,
  MENU_ICON_SIZE,
} from "../components/TableTab.const";

const ICON_LAYOUT = {
  headerPadding: HEADER_PADDING,
  menuIconSize: MENU_ICON_SIZE,
  nameRowHeight: NAME_ROW_HEIGHT,
};

/** Owns everything about TableTab's canvas-rendered header cells: the
 * `drawHeader`/`onItemHovered` callbacks Glide calls into, and the hover
 * tooltip state they drive. Split out from TableTab.jsx because this is
 * the single largest, most self-contained piece of it — name/type-icon/
 * risk-bar/sparkline layout and its hover hit-testing don't touch sort or
 * cell-content state at all, just `columns` and the per-column lookup maps
 * the caller already built from `profile`.
 *
 * Header cells are canvas-rendered by the grid itself (not DOM), so this
 * layout has to be drawn directly into Glide's own header-drawing pass
 * rather than mounted as React nodes — that's the reason this exists as
 * imperative canvas code instead of JSX. */
export function useTableHeaderCanvas({
  columns,
  sort,
  typeByName,
  isRiskableByName,
  isCategoricalByName,
  distributionByName,
  riskByName,
  riskLoading,
}) {
  // Whether the last draw pass had to truncate each column's name — read by
  // the hover tooltip below to decide whether there's anything worth
  // showing. A plain ref, not state: it's written every paint and read only
  // on hover, so it doesn't need to trigger a re-render itself.
  const truncatedRef = useRef({});
  const [hoveredHeader, setHoveredHeader] = useState(null);
  const [hoveredBin, setHoveredBin] = useState(null);

  // Returning `true` tells Glide we drew the whole cell ourselves —
  // including the title, laid out here on its own line above the type — so
  // it shouldn't also draw its default single-line title on top.
  const drawHeader = useCallback(
    (args) => {
      const { ctx, rect, column, theme } = args;
      const innerX = rect.x + HEADER_PADDING;
      const icon = menuIconLocalRect(rect.width, ICON_LAYOUT);
      const sortArrowWidth = 12;
      const nameMaxWidth = icon.x - sortArrowWidth - HEADER_PADDING - 4;
      const fontFamily = theme?.fontFamily ?? "sans-serif";

      ctx.textBaseline = "middle";

      ctx.font = `600 13px ${fontFamily}`;
      ctx.fillStyle = theme?.textHeader ?? "#1a1d24";
      const { text: nameText, truncated } = truncateToWidth(ctx, column.title, nameMaxWidth);
      const nameY = rect.y + NAME_ROW_HEIGHT / 2 + 2;
      ctx.fillText(nameText, innerX, nameY);
      truncatedRef.current[column.id] = truncated;

      if (sort?.column === column.id) {
        drawSortArrow(ctx, rect.x + icon.x - sortArrowWidth / 2, rect.y + NAME_ROW_HEIGHT / 2, sort.direction, theme);
      }

      drawMenuIcon(ctx, rect.x + icon.x, rect.y + icon.y, icon.size, theme);

      const colType = typeByName[column.id];
      const typeRowCenterY = rect.y + NAME_ROW_HEIGHT + TYPE_ROW_HEIGHT / 2;
      if (colType) {
        const iconNode = TYPE_ICON_NODES[colType];
        let textX = innerX;
        if (iconNode) {
          drawIconNode(
            ctx,
            iconNode,
            innerX,
            typeRowCenterY - TYPE_ICON_SIZE / 2,
            TYPE_ICON_SIZE,
            theme?.textLight ?? "#8890a0",
          );
          textX = innerX + TYPE_ICON_SIZE + 4;
        }
        ctx.font = `italic 11px ${fontFamily}`;
        ctx.fillStyle = theme?.textLight ?? "#8890a0";
        ctx.fillText(colType, textX, typeRowCenterY);
      }

      // k-anonymity risk bar, same gray-track/proportional-fill encoding as
      // the Column view's per-card bar (ColumnCard.jsx) — a value label
      // sits to its right, same as ColumnCard's, since the header has just
      // enough width for both. Track stays visible while loading (canvas
      // has no CSS pulse animation to reuse) rather than nothing at all. A
      // categorical column's label also carries its distinct-value count
      // (already computed for the sparkline below, just not shown anywhere
      // in the table before) since k-anonymity alone doesn't tell you how
      // many distinct values that group size is spread across.
      if (isRiskableByName[column.id]) {
        const barY = rect.y + NAME_ROW_HEIGHT + TYPE_ROW_HEIGHT + (RISK_ROW_HEIGHT - RISK_BAR_HEIGHT) / 2;
        const risk = riskByName?.[column.id];
        const cardinality = isCategoricalByName[column.id] ? distributionByName[column.id]?.cardinality : undefined;
        const labelText =
          risk && !riskLoading
            ? cardinality !== undefined
              ? `k=${risk.value} · n=${cardinality}`
              : `k=${risk.value}`
            : "";
        ctx.font = `9px ${fontFamily}`;
        const labelWidth = labelText ? Math.ceil(ctx.measureText(labelText).width) : 0;
        const barWidth = rect.width - HEADER_PADDING * 2 - labelWidth - (labelWidth > 0 ? 4 : 0);
        ctx.fillStyle = theme?.bgHeader ?? "#e1e4e9";
        ctx.fillRect(innerX, barY, barWidth, RISK_BAR_HEIGHT);
        if (!riskLoading && risk) {
          const fillWidth = reciprocalRisk(risk.value) * barWidth;
          ctx.fillStyle = readRiskColorStops().riskColor(reciprocalRisk(risk.value));
          ctx.fillRect(innerX, barY, fillWidth, RISK_BAR_HEIGHT);
          ctx.fillStyle = theme?.textLight ?? "#8890a0";
          ctx.textAlign = "left";
          ctx.fillText(labelText, innerX + barWidth + 4, barY + RISK_BAR_HEIGHT / 2 + 1);
        }
      }

      const bars = barsFor(distributionByName[column.id]);
      const max = Math.max(0, ...bars);
      if (max > 0) {
        const barAreaTop = rect.y + rect.height - SPARK_HEIGHT - 4;
        const sparkWidth = rect.width - HEADER_PADDING * 2;
        const barWidth = sparkWidth / bars.length;
        ctx.fillStyle = "#818cf8";
        bars.forEach((count, i) => {
          const h = (count / max) * SPARK_HEIGHT;
          ctx.fillRect(
            innerX + i * barWidth,
            barAreaTop + (SPARK_HEIGHT - h),
            Math.max(barWidth - 1, 1),
            h,
          );
        });
      }

      return true;
    },
    [distributionByName, typeByName, isRiskableByName, isCategoricalByName, riskByName, riskLoading, sort],
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
      const riskRowTop = NAME_ROW_HEIGHT + TYPE_ROW_HEIGHT;
      if (localY >= riskRowTop && localY < SPARK_TOP && isRiskableByName[column.id]) {
        const risk = riskByName?.[column.id];
        if (risk) {
          const cardinality = isCategoricalByName[column.id] ? distributionByName[column.id]?.cardinality : undefined;
          setHoveredHeader(null);
          setHoveredBin({
            title:
              cardinality !== undefined
                ? `k-anonymity ${risk.value} · ${cardinality} distinct`
                : `k-anonymity ${risk.value}`,
            detail: kAnonymityExplanation(risk.value),
            wrap: true,
            x: args.bounds.x + localX,
            y: args.bounds.y + riskRowTop,
          });
          return;
        }
      }
      if (localY >= SPARK_TOP) {
        const distribution = distributionByName[column.id];
        const bars = barsFor(distribution);
        if (bars.length > 0) {
          const sparkWidth = args.bounds.width - HEADER_PADDING * 2;
          const barWidth = sparkWidth / bars.length;
          const index = Math.min(
            Math.max(Math.floor((localX - HEADER_PADDING) / barWidth), 0),
            bars.length - 1,
          );
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
    [columns, typeByName, distributionByName, isRiskableByName, isCategoricalByName, riskByName],
  );

  return { drawHeader, onItemHovered, hoveredHeader, hoveredBin };
}
