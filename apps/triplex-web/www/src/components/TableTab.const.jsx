// Header layout, top to bottom: name, histogram, type, missing, distinct
// (categorical columns only), privacy risk — the same order and field set
// as TABLE_HEADER_FIELDS in columnFields.js (the ordered subset of
// ColumnCard's fuller info this header shows). Change what the header
// displays by editing that array; change how much room each row gets by
// editing the heights below — see TableTab.jsx's drawHeader for how these
// are used.
//
// The distinct row's height is reserved for every column, not just
// categorical ones — glide-data-grid gives every header cell the same
// HEADER_HEIGHT, so a numeric column just leaves that band blank rather
// than the row disappearing and shifting the risk row up only for some
// columns (same reasoning as the risk row itself, already reserved even
// for non-riskable columns).
export const HEADER_PADDING = 8;
export const NAME_ROW_HEIGHT = 26;
export const TYPE_ICON_SIZE = 14;
export const HIST_ROW_HEIGHT = 34;
export const SPARK_HEIGHT = 26; // the bar area within HIST_ROW_HEIGHT
export const TYPE_ROW_HEIGHT = 22;
export const MISSING_ROW_HEIGHT = 22;
export const DISTINCT_ROW_HEIGHT = 22;
export const RISK_ROW_HEIGHT = 33;
export const RISK_BAR_HEIGHT = 15;

// Row top offsets, computed once so drawHeader and its hover hit-testing
// can't disagree about where a row actually starts.
export const HIST_TOP = NAME_ROW_HEIGHT;
export const TYPE_TOP = HIST_TOP + HIST_ROW_HEIGHT;
export const MISSING_TOP = TYPE_TOP + TYPE_ROW_HEIGHT;
export const DISTINCT_TOP = MISSING_TOP + MISSING_ROW_HEIGHT;
export const RISK_TOP = DISTINCT_TOP + DISTINCT_ROW_HEIGHT;
export const HEADER_HEIGHT = RISK_TOP + RISK_ROW_HEIGHT + 12;

export const MENU_ICON_SIZE = 16;
