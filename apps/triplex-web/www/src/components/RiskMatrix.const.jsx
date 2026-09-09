// The ramp's safe end is the card's own surface color, so a cell with no
// data (e.g. a column paired with itself) needs a color distinct from both
// the ramp and the surface, or it reads as "safest" instead of "not
// applicable".
export const NO_DATA_COLOR = "#c3c2b7";
