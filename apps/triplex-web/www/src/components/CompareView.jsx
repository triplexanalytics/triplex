import { useMemo, useState } from "react";
import ColumnCard from "./ColumnCard";
import InfoTrigger from "./InfoTrigger";
import { DiversityCompare } from "./DiversityCheckSection";
import { AssociationMatrix, AssociationDiffMatrix } from "./AssociationMatrix";
import { distributionDifference } from "../state/columnDelta";

const TABS = [
  { id: "columns", label: "Columns" },
  { id: "associations", label: "Column associations" },
  { id: "privacy", label: "Privacy matrices" },
];
const NO_PATTERN = {};
// A compare card is read-only: none of the original-only actions exist here.
const NO_ACTIONS = {
  anonymizeResult: undefined,
  appliedMethod: undefined,
  declaredType: undefined,
  onBinClick: undefined,
  onCheckPattern: undefined,
  onClearMethod: undefined,
  onDeclareAsString: undefined,
  onOpenMenu: undefined,
  onSelect: undefined,
};

/** `{ label, kind, sample, profile, analysis, excluded?, droppedReasons? }` per side; `analysis` is
 * useDatasetAnalysis's result for that dataset. Side A is the reference (the original); side B is
 * whatever it is compared with (today the synthetic table; later an AI-generated or uploaded one). */
function SideLabels({ left, right }) {
  return (
    <div className="compare-pair compare-labels">
      {[left, right].map((side) => (
        <div key={side.kind} className="compare-label" data-kind={side.kind}>
          {side.label}
        </div>
      ))}
    </div>
  );
}

function CardOrGap({ side, col, name, hoverNames, onColumnHover }) {
  if (!col) {
    return (
      <div className="compare-gap">
        <strong>{name}</strong> is not in the {side.label.toLowerCase()} dataset
        {side.droppedReasons?.[name] ? <span>: {side.droppedReasons[name]}.</span> : "."}
      </div>
    );
  }
  const analysis = side.analysis;
  return (
    <ColumnCard
      col={col}
      risk={{
        value: analysis.columnRisk.riskByName[name],
        loading:
          analysis.columnRisk.riskLoading &&
          (col.inferred_type === "integer" || col.distribution?.kind === "categorical"),
      }}
      selected={false}
      pattern={NO_PATTERN}
      {...NO_ACTIONS}
      readOnly
      highlighted={hoverNames?.rowName === name || hoverNames?.colName === name}
      onHoverChange={onColumnHover}
      subsetCounts={analysis.columnSubsets?.[name]}
    />
  );
}

const pct = (x) => `${x.toFixed(1)}%`;

function DeltaStrip({ name, leftCol, rightCol, left, right, difference }) {
  if (!leftCol || !rightCol) return null;
  const kLeft = left.analysis.columnRisk.riskByName[name]?.value;
  const kRight = right.analysis.columnRisk.riskByName[name]?.value;
  return (
    <div className="compare-delta">
      <span>
        Missing {pct(leftCol.null_percentage)} → {pct(rightCol.null_percentage)}
      </span>
      {kLeft !== undefined && kRight !== undefined && (
        <span>
          k-anonymity {kLeft} → {kRight}
        </span>
      )}
      <span>
        Distribution difference{" "}
        {difference === null || difference === undefined ? "n/a" : difference.toFixed(3)}
        {difference !== null && difference !== undefined && (
          <span className="compare-delta-bar">
            <span style={{ width: `${Math.min(1, difference) * 100}%` }} />
          </span>
        )}
      </span>
      <InfoTrigger
        align="left"
        tooltip="What changed between the two columns. Missing is the share of empty cells. k-anonymity is the size of the smallest group of identical values (larger is safer); it measures each table as it is and is not a privacy guarantee. Distribution difference is the Kolmogorov-Smirnov distance for numbers and the total variation distance for categories: 0 means identical distributions, 1 means completely different."
      />
    </div>
  );
}

export default function CompareView({
  left,
  right,
  hoverNames,
  onHoverNamesChange,
  onColumnHover,
}) {
  const [tab, setTab] = useState("columns");
  const names = useMemo(() => {
    const leftNames = left.profile.columns.map((c) => c.name);
    const rightOnly = right.profile.columns
      .map((c) => c.name)
      .filter((n) => !leftNames.includes(n));
    return [...leftNames, ...rightOnly];
  }, [left.profile, right.profile]);

  const differences = useMemo(() => {
    const out = {};
    for (const name of names) {
      const a = left.profile.columns.find((c) => c.name === name);
      const b = right.profile.columns.find((c) => c.name === name);
      const kind = a?.distribution?.kind;
      if (!a || !b || (kind !== "numeric" && kind !== "categorical")) continue;
      out[name] = distributionDifference(
        name,
        { sample: left.sample, kind, excluded: left.excluded },
        { sample: right.sample, kind: b.distribution?.kind, excluded: right.excluded },
      );
    }
    return out;
  }, [names, left, right]);

  const associationNames = useMemo(() => {
    const a = left.analysis.association?.names ?? [];
    const b = right.analysis.association?.names ?? [];
    return [...a, ...b.filter((n) => !a.includes(n))];
  }, [left.analysis.association, right.analysis.association]);

  const diversity = (side) => ({ ...side.analysis.diversityMatrix });

  return (
    <div className="compare-shell">
      <div className="compare-tabs" role="tablist" aria-label="Comparison sections">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            className={`compare-tab${tab === t.id ? " active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>
      <div className="compare-view" data-tab={tab}>
        {tab === "columns" && (
          <section className="compare-section">
            <h3>Columns</h3>
            <p className="diversity-check-intro">
              One row per column name, {left.label.toLowerCase()} on the left. The line under each
              pair shows what changed.
            </p>
            <div className="compare-sticky">
              <SideLabels left={left} right={right} />
            </div>
            {names.map((name) => {
              const leftCol = left.profile.columns.find((c) => c.name === name);
              const rightCol = right.profile.columns.find((c) => c.name === name);
              return (
                <div className="compare-row" key={name}>
                  <div className="compare-pair">
                    <div className="compare-side">
                      <CardOrGap
                        side={left}
                        col={leftCol}
                        name={name}
                        hoverNames={hoverNames}
                        onColumnHover={onColumnHover}
                      />
                    </div>
                    <div className="compare-side">
                      <CardOrGap
                        side={right}
                        col={rightCol}
                        name={name}
                        hoverNames={hoverNames}
                        onColumnHover={onColumnHover}
                      />
                    </div>
                  </div>
                  <DeltaStrip
                    name={name}
                    leftCol={leftCol}
                    rightCol={rightCol}
                    left={left}
                    right={right}
                    difference={differences[name]}
                  />
                </div>
              );
            })}
          </section>
        )}

        {tab === "privacy" && (
          <section className="compare-section">
            <h3>Privacy matrices</h3>
            <p className="diversity-check-intro">
              The same metric side by side. These measure each table as it is; a lower risk in the
              synthetic table is not a privacy guarantee.
            </p>
            <div className="compare-sticky">
              <SideLabels left={left} right={right} />
            </div>
            <DiversityCompare
              left={diversity(left)}
              right={diversity(right)}
              hoverNames={hoverNames}
              onHoverNamesChange={onHoverNamesChange}
            />
          </section>
        )}

        {tab === "associations" && (
          <section className="compare-section">
            <h3>Column associations</h3>
            <p className="diversity-check-intro">
              How strongly each pair of columns moves together, and how much that changed.
            </p>
            <div className="compare-sticky">
              <SideLabels left={left} right={right} />
            </div>
            <div className="compare-pair">
              <div className="compare-side">
                <AssociationMatrix
                  association={left.analysis.association}
                  names={associationNames}
                  title={`association (${left.label.toLowerCase()})`}
                  hoverNames={hoverNames}
                  onHoverNamesChange={onHoverNamesChange}
                />
              </div>
              <div className="compare-side">
                <AssociationMatrix
                  association={right.analysis.association}
                  names={associationNames}
                  title={`association (${right.label.toLowerCase()})`}
                  hoverNames={hoverNames}
                  onHoverNamesChange={onHoverNamesChange}
                />
              </div>
            </div>
            <div className="compare-pair">
              <div className="compare-side" />
              <div className="compare-side">
                <AssociationDiffMatrix
                  left={left.analysis.association}
                  right={right.analysis.association}
                  names={associationNames}
                  title={`change in association (${left.label.toLowerCase()} → ${right.label.toLowerCase()})`}
                  hoverNames={hoverNames}
                  onHoverNamesChange={onHoverNamesChange}
                />
              </div>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
