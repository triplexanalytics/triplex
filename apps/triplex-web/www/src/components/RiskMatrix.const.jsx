// A cell with no data (e.g. a column paired with itself) needs a color
// distinct from both the ramp and the white card surface, or it reads as
// "safest" instead of "not applicable". A neutral, low-chroma gray — not a
// tint of OCEAN_DRIFT.primary, or it would look like a (mislabeled) point
// on the risk ramp instead of "not part of the scale".
export const NO_DATA_COLOR = "#c4c9d1";
