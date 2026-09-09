// Every plain-language explanation of a risk metric this app shows, in one
// importable place — not scattered across the components that happen to
// display them. Keeps the app-wide voice consistent (lead with what the
// number means to someone who's never heard of the metric, bold the verdict
// fragment, put row/column mechanics last) and gives future work (i18n, a
// glossary page) one file to touch instead of hunting through components.

// Shared k-anonymity tooltip copy — every k-anonymity bar in the app (the
// Column view's per-card bar, the Table tab's header bar) should explain
// the same thing the same way, not just show a number with no context.
export function kAnonymityExplanation(value) {
  const general = (
    <>
      k-anonymity is the size of the smallest group of rows that look identical here after
      generalizing this column. <strong>Higher is safer</strong>, since a bigger group means no
      single row stands out from the rest of it.
    </>
  );
  const specific =
    value <= 1
      ? "At least one value here is unique, and could single out a real person."
      : `Every value shares its generalized range with at least ${value - 1} other row(s).`;
  return (
    <>
      {general} {specific}
    </>
  );
}

// Lead with what the metric means in plain terms; the row/column asymmetry
// is a secondary note at the end, not the headline (it's easy to mistake
// for a bug once noticed, but it's not what a first-time reader needs to
// understand first). "Read more" is inert until the glossary page exists,
// same placeholder pattern as ColumnMenu's method info.
export const L_DIVERSITY_INFO = (
  <>
    k-anonymity only guarantees a group of rows looks alike from the outside. l-diversity checks
    what's inside that group: if everyone in it happens to share the same sensitive value, then
    just knowing which group someone falls into already reveals that value, even without
    knowing which exact row is theirs. <strong>Higher is safer</strong>, since it means every
    group mixes in more distinct values instead of secretly agreeing on one. A row here is the
    column being grouped; a column is the sensitive value checked inside each group, so a cell
    and its mirror image are not expected to match.
    <span className="info-trigger-readmore">Read more…</span>
  </>
);

export const T_CLOSENESS_INFO = (
  <>
    t-closeness checks something subtler than l-diversity: even if a group mixes in several
    sensitive values, the group can still leak information if that mix looks very different from
    the dataset as a whole (say, a group that's mostly one diagnosis when the full dataset is
    evenly split). That gap itself is a clue an attacker can use. <strong>Lower is safer</strong>,
    since it means every group's mix stays close to the overall dataset. As with l-diversity, a
    row is the column being grouped and a column is the sensitive value being checked, so a cell
    and its mirror image are not expected to match.
    <span className="info-trigger-readmore">Read more…</span>
  </>
);

export function lDiversityDetail(value) {
  return value <= 1
    ? "at least one group has only one distinct value, so knowing the group reveals it outright"
    : `every group has at least ${value} distinct values, so no group narrows it down to one`;
}

export function tClosenessDetail(value) {
  return value >= 0.5
    ? "at least one group's mix looks very different from the dataset overall, and that gap itself is a clue"
    : "every group's mix stays close to the dataset overall";
}
