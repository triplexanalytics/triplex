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
      k-anonymity is the size of the smallest group of rows that share the same value here (a
      numeric column is first generalized into a range; a text column is grouped by its exact
      value). <strong>Higher is safer</strong>, since a bigger group means no single row stands
      out from the rest of it.
    </>
  );
  const specific =
    value <= 1
      ? "At least one value here is unique, and could single out a real person."
      : `Every value shares its group with at least ${value - 1} other row(s).`;
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

// Distinct-value count for a categorical/string column — shown bare as
// "Distinct" in ColumnCard and folded into the header's k-anonymity label
// ("k=N · n=M") without ever explaining what it means on its own; this is
// that explanation, in the same lead-with-the-concept style as the others.
export function distinctCountExplanation(count) {
  return (
    <>
      The number of different values this column actually takes on, out of every row. A string
      column has no numeric range to generalize the way an integer column does,
      so its k-anonymity groups are its exact values: <strong>fewer distinct values means
      bigger groups and a safer k-anonymity score</strong>, while a column that's close to one
      distinct value per row is close to unique per row (k=1) even before anonymizing it.
    </>
  );
}

// The dataset-level verdict (the design notes "dataset-level aggregation"
// gap): a single column's k-anonymity only shows how that one column looks
// alone, but a real attacker combines what they already know (a ZIP code
// plus a birth year, say). Maps the three scores back to the EDPB's own
// three criteria for anonymous data (per proposal-datasetcard-adr.md's
// documentation requirement) so a compliance-minded reader has a name for
// what each number is standing in for, not just a bare metric.
export const JOINT_RISK_INFO = (
  <>
    Picking columns here scores them <strong>together</strong>, as one combined quasi-identifier,
    instead of one at a time — two columns that each look safe alone can still single someone out
    once combined. This maps to the EDPB's three criteria for data to count as anonymous: combined
    k-anonymity above 1 supports <strong>No Record Isolation</strong>, l-diversity supports{" "}
    <strong>No Linkage</strong>, and t-closeness supports <strong>No Inference</strong>.
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
