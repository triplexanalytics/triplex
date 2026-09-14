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
// gap): the k-anonymity bars above score one column at a time; this matrix
// scores every *pair* of quasi-identifier columns combined, since a real
// attacker rarely knows only one thing about someone. Same lead-with-the-
// concept, row/column-mechanics-last structure as L_DIVERSITY_INFO/
// T_CLOSENESS_INFO, except row and column here are the same list (every
// quasi-identifier against every other one), not two different roles, so
// there's no "cell and its mirror image aren't expected to match" caveat —
// this matrix is symmetric by construction. Maps to the EDPB's own
// language for anonymous data (per proposal-datasetcard-adr.md's
// plain-language documentation requirement) so a compliance-minded reader
// has a name for what the number stands in for, not just a bare metric.
export const JOINT_K_INFO = (
  <>
    Two columns can each look safe scored alone (a ZIP code shared by thousands of people, a
    birth year shared by millions) and still single out almost everyone once combined — this
    matrix scores every pair of quasi-identifier columns <strong>together</strong> instead of one
    at a time. <strong>Higher is safer</strong>, same direction as k-anonymity itself. It supports
    the EDPB's <strong>No Record Isolation</strong> criterion for data to count as anonymous. Row
    and column are the same list of quasi-identifier columns, so a cell reads the same from either
    side.
    <span className="info-trigger-readmore">Read more…</span>
  </>
);

// beta-likeness/delta-disclosure sharpen t-closeness's
// one aggregate "how different is this group's mix" number into "what's
// the single biggest jump in belief about any one value" — same
// lead-with-the-concept, row/column-mechanics-last structure as
// L_DIVERSITY_INFO/T_CLOSENESS_INFO, since they share the exact same row
// (quasi-identifier column) / column (candidate sensitive column) shape.
export const BETA_LIKENESS_INFO = (
  <>
    t-closeness (left) reports one aggregate number for how different a group's whole mix looks.
    β-likeness reports something sharper: the single biggest jump in belief an attacker gets about
    any <strong>one</strong> sensitive value from being in that group (say, a group where a rare
    diagnosis suddenly looks far more likely than in the dataset overall). It only counts a value
    looking <strong>more</strong> likely inside the group, not less.{" "}
    <strong>Lower is safer</strong>, same direction as t-closeness. A row here is the column being
    grouped; a column is the sensitive value checked inside each group, so a cell and its mirror
    image are not expected to match.
    <span className="info-trigger-readmore">Read more…</span>
  </>
);

export const DELTA_DISCLOSURE_INFO = (
  <>
    δ-disclosure is β-likeness's sibling: the same "single biggest belief jump" idea, but counting
    a group making a value look <strong>less</strong> likely too, not just more — ruling something
    out with confidence is also a leak, even though it never shows up in β-likeness.{" "}
    <strong>Lower is safer</strong>, same direction as β-likeness and t-closeness. As with those, a
    row is the column being grouped and a column is the sensitive value being checked, so a cell
    and its mirror image are not expected to match.
    <span className="info-trigger-readmore">Read more…</span>
  </>
);

export function betaLikenessDetail(value) {
  return value <= 0
    ? "no group ever makes a sensitive value look more likely than the dataset overall"
    : "at least one group makes a sensitive value look meaningfully more likely than the dataset overall";
}

export function deltaDisclosureDetail(value) {
  return value <= 0
    ? "no group's belief in any sensitive value shifts from the dataset overall, in either direction"
    : "at least one group shifts belief in a sensitive value — up or down — from the dataset overall";
}

export function jointKDetail(value) {
  return value <= 1
    ? "combining these two columns still leaves at least one row unique, which could single out a real person"
    : `combined, every row still shares its group with at least ${value - 1} other row(s)`;
}

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

// SUDA/DIS-SUDA and EBM: the only row-level (not
// column or column-pair) score in the app — every matrix above answers "how
// safe is this column/pair," this answers "which specific rows are most
// identifiable." Same voice as the matrix explanations above, adapted for a
// ranked list instead of a grid. Two techniques, switchable — see
// `RowRiskSection`'s model picker — so this returns different copy for each
// rather than one static blurb that only describes SUDA.
export function rowRiskInfo(model) {
  if (model === "ebm") {
    return (
      <>
        Every score above is about a <strong>column</strong> or a pair of columns. This is about
        individual <strong>rows</strong>: an Explainable Boosting Machine (EBM) trained on the
        quasi-identifier columns to predict how rare each row's combination of values is —{" "}
        <strong>higher is riskier</strong>, and rows with a common, widely-shared combination
        score close to 0. Unlike SUDA's exact combinatorial search, this is a learned estimate, so
        it's not on the same numeric scale as SUDA's score.
        <span className="info-trigger-readmore">Read more…</span>
      </>
    );
  }
  return (
    <>
      Every score above is about a <strong>column</strong> or a pair of columns. This is about
      individual <strong>rows</strong>: for each row, SUDA finds the smallest combination of the
      quasi-identifier columns whose values make that row unique in the dataset — a row unique on
      just two columns is riskier than one that needs six columns to stand out, and a row with
      several different minimal combinations that single it out is riskier still.{" "}
      <strong>Lower is safer</strong>, and 0 means the row was never part of any minimal unique
      combination. DIS-SUDA turns each row's score into its share of the dataset's total
      disclosure risk, so the shares below add up to 100%.
      <span className="info-trigger-readmore">Read more…</span>
    </>
  );
}

export function rowRiskDetail(score, model) {
  if (model === "ebm") {
    return score <= 0
      ? "this row's combination of quasi-identifier values was predicted unremarkable"
      : "this row's combination of quasi-identifier values was predicted rare";
  }
  return score <= 0
    ? "this row was never uniquely identified by any combination of the quasi-identifier columns checked"
    : "this row is uniquely identified by at least one combination of the quasi-identifier columns checked";
}
