import { useState } from "react";
import { Info } from "lucide-react";

// Tallest tooltip we draw (about 360px incl. the gap): the room needed above to open upward.
const TOOLTIP_ROOM = 370;

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
  // Opens upward by default; flips downward when there is not room above inside the nearest
  // scrolling ancestor (or the window) and there is more room below, so a tooltip is not cut off
  // at the top of a scroll area.
  const [dir, setDir] = useState("up");
  const place = (event) => {
    const rect = event.currentTarget.getBoundingClientRect();
    let box = event.currentTarget.parentElement;
    while (box && !/(auto|scroll)/.test(getComputedStyle(box).overflowY)) box = box.parentElement;
    const bounds = box ? box.getBoundingClientRect() : { top: 0, bottom: window.innerHeight };
    const above = rect.top - Math.max(bounds.top, 0);
    const below = Math.min(bounds.bottom, window.innerHeight) - rect.bottom;
    setDir(above >= TOOLTIP_ROOM || above >= below ? "up" : "down");
  };
  return (
    <span
      className={`info-trigger-glyph${wide ? " info-trigger-wide" : ""}`}
      tabIndex={0}
      onMouseEnter={place}
      onFocus={place}
    >
      {children ?? <Info size={14} strokeWidth={2} aria-hidden="true" />}
      {/* .info-trigger-tooltip is the hoverable hit box (padded to reach
       * the trigger with no dead gap); .info-trigger-tooltip-box is the
       * visible card inside it. Keeping them separate is what lets the
       * padding bridge the gap without changing how the tooltip looks. */}
      <span className={`info-trigger-tooltip info-trigger-tooltip-${align}`} data-dir={dir}>
        <span className="info-trigger-tooltip-box">{tooltip}</span>
      </span>
    </span>
  );
}
