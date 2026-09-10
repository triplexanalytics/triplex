/** Shared, component-agnostic helpers — anything used by more than one
 * component lives here instead of being redefined per-file. */

/** Triggers a browser download of in-memory content — used by every
 * "Download ..." button (contract/recipe YAML, anonymized data). */
export function downloadBlob(content, filename, mime) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

/** Trims `text` to fit `maxWidth`, appending an ellipsis — measured with the
 * canvas context's own current font, so it matches exactly what gets drawn
 * (a CSS `text-overflow: ellipsis` guess would drift from canvas metrics). */
export function truncateToWidth(ctx, text, maxWidth) {
  if (ctx.measureText(text).width <= maxWidth) {
    return { text, truncated: false };
  }
  let lo = 0;
  let hi = text.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (ctx.measureText(text.slice(0, mid) + "…").width <= maxWidth) {
      lo = mid;
    } else {
      hi = mid - 1;
    }
  }
  return { text: text.slice(0, lo) + "…", truncated: true };
}

/** The menu icon's position within a header cell, local to the cell's own
 * top-left corner — used both to draw it and to hit-test a click against
 * it, so the two can never drift out of sync with each other. Takes the
 * layout constants as parameters rather than importing them, so this stays
 * generic instead of implicitly coupled to `TableTab.const.jsx`. */
export function menuIconLocalRect(cellWidth, { headerPadding, menuIconSize, nameRowHeight }) {
  return {
    x: cellWidth - headerPadding - menuIconSize,
    y: (nameRowHeight - menuIconSize) / 2,
    size: menuIconSize,
  };
}

/** A small filled triangle centered on (cx, cy) — the sort indicator.
 * Drawn as a path rather than the "▲"/"▼" glyphs: those render inconsistently
 * heavy/lopsided across fonts at small sizes, where a vector shape doesn't. */
export function drawSortArrow(ctx, cx, cy, direction, theme) {
  const w = 8;
  const h = 6;
  ctx.fillStyle = theme?.accentColor ?? "#4f46e5";
  ctx.beginPath();
  if (direction === "asc") {
    ctx.moveTo(cx, cy - h / 2);
    ctx.lineTo(cx - w / 2, cy + h / 2);
    ctx.lineTo(cx + w / 2, cy + h / 2);
  } else {
    ctx.moveTo(cx, cy + h / 2);
    ctx.lineTo(cx - w / 2, cy - h / 2);
    ctx.lineTo(cx + w / 2, cy - h / 2);
  }
  ctx.closePath();
  ctx.fill();
}

export function drawMenuIcon(ctx, x, y, size, theme) {
  const cx = x + size / 2;
  const gap = size / 3;
  ctx.fillStyle = theme?.textLight ?? "#8890a0";
  for (let i = 0; i < 3; i++) {
    ctx.beginPath();
    ctx.arc(cx, y + gap * i + gap / 2, 1.6, 0, Math.PI * 2);
    ctx.fill();
  }
}
