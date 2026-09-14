import { useCallback, useRef, useState } from "react";
import { barsFor, binTooltip } from "../state/binTooltip";
import { truncateToWidth, menuIconLocalRect, drawMenuIcon, drawSortArrows } from "../utils";
import { TYPE_ICON_NODES, drawIconNode } from "../state/typeIconNodes";
import { readRiskColorStops, HISTOGRAM_COLOR, reciprocalRisk } from "../state/palette";
import { kAnonymityExplanation, distinctCountExplanation } from "../state/riskExplanations";
import {
  HEADER_PADDING,
  NAME_ROW_HEIGHT,
  TYPE_ICON_SIZE,
  HIST_TOP,
  HIST_ROW_HEIGHT,
  SPARK_HEIGHT,
  TYPE_TOP,
  TYPE_ROW_HEIGHT,
  MISSING_TOP,
  MISSING_ROW_HEIGHT,
  DISTINCT_TOP,
  DISTINCT_ROW_HEIGHT,
  RISK_TOP,
  RISK_ROW_HEIGHT,
  RISK_BAR_HEIGHT,
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
 * Row order (name, histogram, type, missing, risk) mirrors
 * TABLE_HEADER_FIELDS in columnFields.js — ColumnCard's fuller info,
 * reduced to what fits here, same order. Row heights/offsets live in
 * TableTab.const.jsx so drawHeader and this hook's hover hit-testing can't
 * silently disagree about where a row starts.
 *
 * Header cells are canvas-rendered by the grid itself (not DOM), so this
 * layout has to be drawn directly into Glide's own header-drawing pass
 * rather than mounted as React nodes — that's the reason this exists as
 * imperative canvas code instead of JSX. */
export function useTableHeaderCanvas({
  columns,
  sort,
  typeByName,
  missingByName,
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
      const fontFamily = theme?.fontFamily ?? "sans-serif";
      // Matched to ColumnCard's actual CSS, not an independently chosen
      // scale: h3 (0.92rem, bold) for the name, the <dl> (0.78rem, weight
      // 500) for type/missing, .risk-bar-label (0.72rem) for the risk
      // caption — see palette.js's readGlideHeaderTheme doc comment for the
      // color side of this same match.
      const nameFont = `700 13px ${fontFamily}`;
      const statFont = `500 11px ${fontFamily}`;
      const captionFont = `10px ${fontFamily}`;
      const nameColor = theme?.textDark ?? "#1a1d24";
      const faintColor = theme?.textLight ?? "#8890a0"; // --text-faint: the type icon only
      const mutedColor = theme?.textMedium ?? "#5b6270"; // --text-muted: every stat's label AND value, plus the risk caption

      ctx.textBaseline = "middle";

      // ColumnCard's <dl> is a CSS grid (grid-template-columns: auto 1fr):
      // every dt sits in a label column sized to the widest label, every dd
      // starts at the same fixed x regardless of its own row's label
      // length. Replicate that instead of each row computing its own
      // label-width gap (misaligns "Type"/"Missing"/"Privacy risk"'s values
      // the moment the labels aren't all the same width).
      ctx.font = statFont;
      const labelColumnWidth = Math.max(
        ctx.measureText("Type").width,
        ctx.measureText("Missing").width,
        ctx.measureText("Distinct").width,
        ctx.measureText("Privacy risk").width,
      );
      const valueX = innerX + labelColumnWidth + 6;

      // --- name row --- (ColumnCard's h3: bold, --text)
      const nameRowCenterY = rect.y + NAME_ROW_HEIGHT / 2;
      const nameMaxWidth = icon.x - sortArrowWidth - HEADER_PADDING - 4;
      ctx.font = nameFont;
      ctx.fillStyle = nameColor;
      const { text: nameText, truncated } = truncateToWidth(ctx, column.title, nameMaxWidth);
      ctx.fillText(nameText, innerX, nameRowCenterY + 2);
      truncatedRef.current[column.id] = truncated;

      drawSortArrows(
        ctx,
        rect.x + icon.x - sortArrowWidth / 2,
        nameRowCenterY,
        sort?.column === column.id ? sort.direction : null,
        theme,
      );
      drawMenuIcon(ctx, rect.x + icon.x, rect.y + icon.y, icon.size, theme);

      // --- histogram row ---
      const bars = barsFor(distributionByName[column.id]);
      const max = Math.max(0, ...bars);
      if (max > 0) {
        const baseline = rect.y + HIST_TOP + HIST_ROW_HEIGHT - 4;
        const sparkWidth = rect.width - HEADER_PADDING * 2;
        const barWidth = sparkWidth / bars.length;
        // One flat, uniform color for every bar — no gradient, and no
        // per-bar value mapping either: height alone carries magnitude.
        ctx.fillStyle = HISTOGRAM_COLOR;
        bars.forEach((count, i) => {
          const h = (count / max) * SPARK_HEIGHT;
          const barX = innerX + i * barWidth;
          const barY = baseline - h;
          const w = Math.max(barWidth - 1, 1);
          const radius = Math.min(2, w / 2, h / 2);
          ctx.beginPath();
          ctx.roundRect(barX, barY, w, h, [radius, radius, 0, 0]);
          ctx.fill();
        });
      }

      // --- type row --- ColumnCard's <dt>Type</dt> label ahead of the
      // dd.type-with-icon value (icon in --text-faint, everything else in
      // --text-muted, same as every other stat).
      const colType = typeByName[column.id];
      const typeRowCenterY = rect.y + TYPE_TOP + TYPE_ROW_HEIGHT / 2;
      if (colType) {
        ctx.font = statFont;
        ctx.fillStyle = mutedColor;
        ctx.fillText("Type", innerX, typeRowCenterY + 1);
        let textX = valueX;

        const iconNode = TYPE_ICON_NODES[colType];
        if (iconNode) {
          drawIconNode(ctx, iconNode, textX, typeRowCenterY - TYPE_ICON_SIZE / 2, TYPE_ICON_SIZE, faintColor);
          textX += TYPE_ICON_SIZE + 6;
        }
        ctx.fillText(colType, textX, typeRowCenterY + 1);
      }

      // --- missing row --- ("Missing" label and its value are the same
      // --text-muted tone, like every dt/dd pair in ColumnCard's <dl> — not
      // a label/value two-tone, that was this file's own invented
      // hierarchy, not what the card does)
      const missingText = missingByName[column.id];
      if (missingText !== undefined) {
        const missingRowCenterY = rect.y + MISSING_TOP + MISSING_ROW_HEIGHT / 2;
        ctx.font = statFont;
        ctx.fillStyle = mutedColor;
        ctx.fillText("Missing", innerX, missingRowCenterY + 1);
        ctx.fillText(missingText, valueX, missingRowCenterY + 1);
      }

      // --- distinct row --- ColumnCard's <dt>Distinct</dt>, categorical
      // columns only (a numeric column has no equivalent stat — the row's
      // space still reserves, it just stays blank, same as the risk row
      // does for a non-riskable column).
      if (isCategoricalByName[column.id]) {
        const cardinality = distributionByName[column.id]?.cardinality;
        if (cardinality !== undefined) {
          const distinctRowCenterY = rect.y + DISTINCT_TOP + DISTINCT_ROW_HEIGHT / 2;
          ctx.font = statFont;
          ctx.fillStyle = mutedColor;
          ctx.fillText("Distinct", innerX, distinctRowCenterY + 1);
          ctx.fillText(String(cardinality), valueX, distinctRowCenterY + 1);
        }
      }

      // --- risk row --- ColumnCard's order: the bar (.risk-bar-track/-fill)
      // first, its label (.risk-bar-label — small, --text-muted, a caption,
      // not a headline) after/below it, not the reverse this used to draw.
      // A full-width pill (rounded via the canvas's native roundRect, no
      // manual bezier path); track stays visible while loading (canvas has
      // no CSS pulse animation to reuse) rather than nothing at all. A
      // categorical column's label also carries its distinct-value count
      // since k-anonymity alone doesn't say how many distinct values that
      // group size is spread across.
      if (isRiskableByName[column.id]) {
        const risk = riskByName?.[column.id];
        const barY = rect.y + RISK_TOP + 4;
        const barWidth = rect.x + rect.width - HEADER_PADDING - valueX;
        const radius = RISK_BAR_HEIGHT / 2;

        ctx.font = statFont;
        ctx.fillStyle = mutedColor;
        ctx.textAlign = "left";
        ctx.fillText("Privacy risk", innerX, barY + RISK_BAR_HEIGHT / 2);

        ctx.fillStyle = theme?.bgCellMedium ?? "#f1f3f6";
        ctx.beginPath();
        ctx.roundRect(valueX, barY, barWidth, RISK_BAR_HEIGHT, radius);
        ctx.fill();
        if (!riskLoading && risk) {
          const fillWidth = Math.max(RISK_BAR_HEIGHT, reciprocalRisk(risk.value) * barWidth);
          ctx.fillStyle = readRiskColorStops().riskColor(reciprocalRisk(risk.value));
          ctx.beginPath();
          ctx.roundRect(valueX, barY, fillWidth, RISK_BAR_HEIGHT, radius);
          ctx.fill();
        }
        const valueText = risk && !riskLoading ? `k=${risk.value}` : "";
        if (valueText) {
          ctx.font = captionFont;
          ctx.fillStyle = mutedColor;
          ctx.fillText(valueText, valueX, barY + RISK_BAR_HEIGHT + 10);
        }
      }

      return true;
    },
    [distributionByName, typeByName, missingByName, isRiskableByName, isCategoricalByName, riskByName, riskLoading, sort],
  );

  // Same header hover callback drives two mutually-exclusive tooltips: the
  // truncated-name one and the histogram-bin/risk one — whichever region
  // the cursor is actually over.
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
      if (localY >= RISK_TOP && isRiskableByName[column.id]) {
        const risk = riskByName?.[column.id];
        if (risk) {
          setHoveredHeader(null);
          setHoveredBin({
            title: `k-anonymity ${risk.value}`,
            detail: kAnonymityExplanation(risk.value),
            wrap: true,
            x: args.bounds.x + localX,
            y: args.bounds.y + RISK_TOP,
          });
          return;
        }
      }
      if (localY >= DISTINCT_TOP && localY < RISK_TOP && isCategoricalByName[column.id]) {
        const cardinality = distributionByName[column.id]?.cardinality;
        if (cardinality !== undefined) {
          setHoveredHeader(null);
          setHoveredBin({
            title: `${cardinality} distinct`,
            detail: distinctCountExplanation(cardinality),
            wrap: true,
            x: args.bounds.x + localX,
            y: args.bounds.y + DISTINCT_TOP,
          });
          return;
        }
      }
      if (localY >= HIST_TOP && localY < TYPE_TOP) {
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
