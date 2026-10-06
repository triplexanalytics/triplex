// The app's one color-scheme module: every data-viz/risk color in the app
// (the header sparkline, the k-anonymity/risk bars, the l-diversity and
// t-closeness heatmaps, glide-data-grid's own header chrome) reads from
// here instead of picking colors inline, per the dataviz skill's
// color-formula method — a job (sequential, diverging, status...) gets one
// hue, computed as steps of lightness, never eyeballed.

// "Ocean Drift" — the design system this app is moving to (deep oceanic
// blues/teals, a clean white surface), fixed hex values, not theme tokens:
// the app is light-only by design, so these stay constant rather than
// varying with anything. Every role the source doc defines, so later steps
// have a name to reach for instead of a new inline hex.
export const OCEAN_DRIFT = {
  primary: "#0f4c81",
  secondary: "#1a73b8",
  accent: "#38b2ac",
  background: "#f0f9ff",
  surface: "#ffffff",
  text: "#1a202c",
};

// `OCEAN_DRIFT.primary` at a given alpha, as a CSS color -- one spelling of
// "15, 76, 129" instead of two copies (this module's own `borderColor`
// below, and RiskMatrix.jsx's cross-highlight band) silently drifting if
// either is ever edited alone.
export function oceanDriftAlpha(alpha) {
  return `rgba(15, 76, 129, ${alpha})`;
}

// The header sparkline's uniform bar color. Not OCEAN_DRIFT.accent: computed
// contrast against the spec's own background/surface is only 2.4-2.6:1,
// under the dataviz skill's >=3:1 mark floor. OCEAN_DRIFT.secondary clears
// 4.7-5.0:1 against the same surfaces and is still a clearly "accent-ish"
// blue, so it's the one actually used for a small bar mark.
export const HISTOGRAM_COLOR = OCEAN_DRIFT.secondary;

// The "full dataset, before this filter" baseline bar HeaderSparkline draws
// once a row filter (useRowFilter.js) is active anywhere in the app --
// coordinated-view brushing: every card keeps showing its own bars at full
// (unfiltered) height in this muted gray, with a shorter HISTOGRAM_COLOR
// overlay on top sized to how many of that bar's own rows survive the
// filter. Deliberately not held to HISTOGRAM_COLOR's >=3:1 mark-contrast
// floor -- once overlaid, this is the *receded* layer, not the primary data
// mark (the overlay is), the same "faded background, one real mark on top"
// relationship a track/fill risk bar already has (ColumnCard's
// .risk-bar-track).
export const HISTOGRAM_BASELINE_COLOR = "#d7dbe2";

// glide-data-grid ships its own default theme (light gray #F7F7F8 header,
// Inter font, an unrelated #4F5DFF accent) and nothing in this app ever
// overrode it — TableTab's <DataEditor> took no `theme` prop, so every
// custom color drawn in the header (the sparkline, the risk bar) sat on top
// of a background and font family this module never touched, which is the
// actual reason the header read as inconsistent/hard to read. This is the
// override, passed as `theme` in TableTab.jsx so glide's own chrome
// (header background, borders, the default text/accent it uses anywhere
// this app doesn't draw its own) and this module's hand-drawn colors
// finally agree.
//
// A function, not a static object: every role below reads live off the
// document, the same document ColumnCard's CSS is painted against, so this
// module never hardcodes a duplicate of styles.css's hex values and drifts
// from them over time. bgHeader*/bgCell/accentColor/borderColor read the
// --table-header-bg*/--accent/--table-border tokens styles.css defines (a
// second copy of the same hex by necessity -- CSS custom properties can't
// import a JS constant -- so keep them in sync by hand if either changes).
/** `dataset` ("original" | "synthetic" | "ai") picks the header background: the same tint as
 * that dataset's tab (`--table-header-bg-<dataset>` etc. in styles.css), falling back to the
 * base tokens. */
export function readGlideHeaderTheme(dataset = "original") {
  const style = getComputedStyle(document.documentElement);
  const cssVar = (name, fallback) => style.getPropertyValue(name).trim() || fallback;
  const headerVar = (name, fallback) => cssVar(`${name}-${dataset}`, cssVar(name, fallback));
  // The exact three grayscale tokens ColumnCard's CSS already uses, read
  // live rather than duplicated as hex, so the canvas header's text
  // hierarchy matches the DOM card's pixel-for-pixel:
  // --text for a headline (the card's h3, the header's name row),
  // --text-muted for the card's entire <dl> (every stat's label AND value
  // share this one tone — not a label/value two-tone, that was this
  // module's own invented hierarchy, not what the card does) plus the
  // risk-bar-label caption, --text-faint for just the type icon.
  const text = cssVar("--text", "#1a1d24");
  const textMuted = cssVar("--text-muted", "#5b6270");
  const textFaint = cssVar("--text-faint", "#8890a0");
  const surfaceAlt = cssVar("--surface-alt", "#f1f3f6");
  const surface = cssVar("--surface", OCEAN_DRIFT.surface);
  const accent = cssVar("--accent", OCEAN_DRIFT.primary);
  const accentFg = cssVar("--accent-text", "#ffffff");

  return {
    bgHeader: headerVar("--table-header-bg", OCEAN_DRIFT.background),
    bgHeaderHasFocus: headerVar("--table-header-bg-focus", "#dcefff"),
    bgHeaderHovered: headerVar("--table-header-bg-hover", "#e5f2ff"),
    bgCell: surface, // every other card/panel's own surface token, not a fixed white hex of its own
    bgCellMedium: surfaceAlt, // the risk bar's track — same gray ColumnCard's .risk-bar-track uses, not the header background (or an empty bar reads as "no bar")
    textDark: text,
    textHeader: text,
    textMedium: textMuted,
    textLight: textFaint,
    // Read (not derived on the fly) by useTableHeaderCanvas's drawHeader: when
    // Glide marks a header `isSelected` it fills the whole cell with solid
    // accentColor first (data-grid-render.header.js), so the text drawn on
    // top has to switch to this instead of textDark/textMedium/textLight or
    // it's dark-on-dark-navy and unreadable. Same --accent-text token
    // button.primary already uses for the same "text sitting on solid accent"
    // case.
    accentFg,
    accentColor: accent, // the app's one unified accent token (styles.css), not a fixed light-only hex
    borderColor: cssVar("--table-border", oceanDriftAlpha(0.16)), // OCEAN_DRIFT.primary at low alpha, not glide's neutral gray
    fontFamily: '-apple-system, "Segoe UI", system-ui, Roboto, sans-serif', // match the app's own body font, not glide's Inter default
  };
}

function hexToRgb(hex) {
  const n = parseInt(hex.replace("#", ""), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

function lerpRgb(a, b, t) {
  return [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)];
}

function rgbToCss(rgb) {
  const [r, g, b] = rgb;
  return `rgb(${Math.round(r)}, ${Math.round(g)}, ${Math.round(b)})`;
}

// Under Ocean Drift, red doesn't work as the risk ramp's hue: every
// light-enough tint of OCEAN_DRIFT.accent (the palette's only teal) fails
// the dataviz skill's >=3:1 mark-contrast floor against the surfaces this
// app actually draws it on. A tint of OCEAN_DRIFT.primary does clear that
// floor, so the ramp stays a single hue (primary, light->dark) instead of
// reaching for a color outside the palette.
//
// Two different mixes, not one, because "light enough" depends on what the
// color sits on top of and how it's drawn:
//   - PILL_SAFE_MIX: the header's k-anonymity bar is a thin fill inside a
//     track painted the header's own background color — the fill itself is
//     the only thing distinguishing "there's a value here" from "empty
//     track", so it needs real contrast against that background. 0.3
//     clears 3.82:1 against OCEAN_DRIFT.background — computed, not
//     eyeballed. A lighter mix here (tested up to 0.55) drops under 3:1.
//   - MATRIX_SAFE_MIX: a heatmap cell is a large, bordered square (its own
//     stroke already marks its shape) sitting on the page's white surface,
//     not a thin mark that disappears without fill contrast — so it can
//     and should go all the way to white, the way any sequential heatmap's
//     "low" end does, relying on the cell's own border (not fill contrast)
//     to stay legible against the page.
const PILL_SAFE_MIX = 0.3;
const MATRIX_SAFE_MIX = 1;

/** Returns a `riskColor(t)` function for `t` in [0, 1] (0 = safest, 1 =
 * riskiest) plus the two raw hex stops for drawing the legend gradient — a
 * single-hue sequential ramp in OCEAN_DRIFT.primary. `variant` picks which
 * safe-end mix applies (see PILL_SAFE_MIX/MATRIX_SAFE_MIX above); defaults
 * to the header pill's, the original caller. Fixed hex values, like the
 * rest of the Ocean Drift palette. */
export function readRiskColorStops(variant = "pill") {
  const safeMix = variant === "matrix" ? MATRIX_SAFE_MIX : PILL_SAFE_MIX;
  const primary = hexToRgb(OCEAN_DRIFT.primary);
  const stops = [lerpRgb(primary, [255, 255, 255], safeMix), primary];

  function riskColor(t) {
    const clamped = Math.max(0, Math.min(1, t ?? 0));
    return rgbToCss(lerpRgb(stops[0], stops[1], clamped));
  }

  return { riskColor, hexStops: stops.map(rgbToCss) };
}

/** Normalizes an unbounded "bigger is safer" count (k-anonymity,
 * l-diversity) into a 0-1 risk fraction: 1 -> full risk, larger values ->
 * diminishing risk, asymptotically toward 0. Shared so the per-column
 * k-anonymity bar and the l-diversity heatmap read the same value the same
 * way. */
export function reciprocalRisk(value) {
  return value <= 0 ? 1 : Math.min(1, 1 / value);
}
