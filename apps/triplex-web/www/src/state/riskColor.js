// Interpolates the app's own surface/error tokens into a white(-ish)-to-red
// risk ramp — a single hue, light->dark, the dataviz method's actual
// default sequential treatment (green-to-red was rejected: it's the
// classic colorblind-unsafe pairing, on top of not being a single hue).
// The low end anchors to the card surface color rather than a literal
// white so a safe cell recedes into the background in both light and dark
// mode, instead of a hardcoded white looking wrong on a dark surface.

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

function rgbToCss([r, g, b]) {
  return `rgb(${Math.round(r)}, ${Math.round(g)}, ${Math.round(b)})`;
}

/** Reads the current --surface/--error-text values off the document (so
 * they already reflect light/dark mode) and returns a `riskColor(t)`
 * function for `t` in [0, 1] (0 = safest, 1 = riskiest) plus the two raw
 * hex stops for drawing the legend gradient. */
export function readRiskColorStops() {
  const style = getComputedStyle(document.documentElement);
  const safe = style.getPropertyValue("--surface").trim() || "#ffffff";
  const risky = style.getPropertyValue("--error-text").trim() || "#b3261e";
  const stops = [hexToRgb(safe), hexToRgb(risky)];

  function riskColor(t) {
    const clamped = Math.max(0, Math.min(1, t ?? 0));
    return rgbToCss(lerpRgb(stops[0], stops[1], clamped));
  }

  return { riskColor, hexStops: [safe, risky] };
}

/** Normalizes an unbounded "bigger is safer" count (k-anonymity,
 * l-diversity) into a 0-1 risk fraction: 1 -> full risk, larger values ->
 * diminishing risk, asymptotically toward 0. Shared so the per-column
 * k-anonymity bar and the l-diversity heatmap read the same value the same
 * way. */
export function reciprocalRisk(value) {
  return value <= 0 ? 1 : Math.min(1, 1 / value);
}
