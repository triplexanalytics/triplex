// The column-info fields ColumnCard (Dataset Anonymity Analysis tab) and
// the Table tab's canvas-drawn column header both show, kept in one place
// so the header's reduced view stays an explicit, ordered subset of the
// card's fuller one instead of two components that drifted into looking
// different by accident (see palette.js's history: HISTOGRAM_COLOR was
// already this kind of drift once).
//
// This can't be one shared React component — the header is glide-data-grid
// canvas drawing, ColumnCard is DOM/CSS — but the *data* both draw from
// only needs to be defined once.

// The header's field order, top to bottom. A caller changes what the
// header shows by editing this array, not by restructuring drawHeader.
// "distinct" mirrors ColumnCard: only shown for categorical columns.
export const TABLE_HEADER_FIELDS = ["name", "histogram", "type", "missing", "distinct", "risk"];

/** ColumnCard shows this as `col.null_percentage.toFixed(1)}%`; the header
 * needs the exact same value formatted the exact same way. */
export function formatMissingPercent(col) {
  return `${col.null_percentage.toFixed(1)}%`;
}

/** The type ColumnCard's "Type"/"Detected" rows would show, collapsed to
 * the single value the header has room for: the detected semantic type
 * (iban, address, ...) when there is one, else the bare primitive type —
 * also the lookup key for TYPE_ICON_NODES/TypeIcon, so the icon and the
 * label never disagree about which type this is. */
export function primaryTypeOf(col) {
  return col.semantic_type ?? col.inferred_type;
}
