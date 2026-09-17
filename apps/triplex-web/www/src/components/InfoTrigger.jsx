import { Info } from "lucide-react";

/** A hover/focus tooltip trigger. Dims itself via `color`, not `opacity` —
 * `opacity` composites the whole subtree, so it would also fade the
 * tooltip child, not just the trigger.
 *
 * `children` is the trigger itself: omit it for the default glyph (an SVG
 * `Info` icon, not the Unicode "ⓘ" character — that glyph has no real
 * vector coverage in most system fonts and falls back to a pixelated
 * bitmap at this size), or pass a bigger element to make it the whole
 * trigger, with `wide` so it fills its container instead of sizing to the
 * glyph. `tooltip` can be a string or richer JSX (e.g. a trailing link).
 *
 * `align` picks which edge of the tooltip anchors to the trigger: "center"
 * (default) suits a trigger with room on both sides; "left" keeps the
 * tooltip from spilling off the left edge of the viewport for a trigger
 * that sits near it (there's no scroll-aware clamping here, so a fixed
 * anchor per known usage is the tradeoff instead of measuring at runtime). */
export default function InfoTrigger({ tooltip, children = null, wide = false, align = "center" }) {
  return (
    <span className={`info-trigger-glyph${wide ? " info-trigger-wide" : ""}`} tabIndex={0}>
      {children ?? <Info size={14} strokeWidth={2} aria-hidden="true" />}
      {/* .info-trigger-tooltip is the hoverable hit box (padded to reach
       * the trigger with no dead gap); .info-trigger-tooltip-box is the
       * visible card inside it. Keeping them separate is what lets the
       * padding bridge the gap without changing how the tooltip looks. */}
      <span className={`info-trigger-tooltip info-trigger-tooltip-${align}`}>
        <span className="info-trigger-tooltip-box">{tooltip}</span>
      </span>
    </span>
  );
}
