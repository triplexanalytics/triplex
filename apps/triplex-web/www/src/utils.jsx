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

/** The row-data formats the browser build can both read and write (see
 * `apps/triplex-web/README.md`'s Scope section — no `.parquet` here, it
 * needs a C toolchain `wasm32-unknown-unknown` doesn't have). Shared by the
 * download-format picker (Navbar) and `downloadData`'s mime lookup, so the
 * two can't drift out of sync on which formats exist. */
export const DOWNLOAD_FORMATS = ["csv", "tsv", "jsonl"];

const MIME_BY_EXTENSION = {
  csv: "text/csv",
  tsv: "text/tab-separated-values",
  jsonl: "application/x-ndjson",
};

export function mimeForExtension(extension) {
  return MIME_BY_EXTENSION[extension] ?? "application/octet-stream";
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

/** A small filled triangle centered on (cx, cy), pointing "up" or "down". */
function fillTriangle(ctx, cx, cy, direction) {
  const w = 7;
  const h = 5;
  ctx.beginPath();
  if (direction === "up") {
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

/** The sort indicator: an up/down arrow pair centered on (cx, cy), stacked
 * vertically — every sortable column shows this, not just the currently
 * sorted one, so "click to sort" reads as a standing affordance rather than
 * something that only appears after the fact. `direction` ("asc" | "desc" |
 * null for unsorted) colors whichever arrow matches the accent color and
 * leaves the other (both, if unsorted) a neutral gray. Drawn as paths
 * rather than the "▲"/"▼" glyphs: those render inconsistently
 * heavy/lopsided across fonts at small sizes, where a vector shape doesn't. */
export function drawSortArrows(ctx, cx, cy, direction, theme) {
  const gap = 2;
  const grayColor = theme?.textLight ?? "#8890a0";
  const activeColor = theme?.accentColor ?? "#4f46e5";

  ctx.fillStyle = direction === "asc" ? activeColor : grayColor;
  fillTriangle(ctx, cx, cy - gap / 2 - 2.5, "up");

  ctx.fillStyle = direction === "desc" ? activeColor : grayColor;
  fillTriangle(ctx, cx, cy + gap / 2 + 2.5, "down");
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
