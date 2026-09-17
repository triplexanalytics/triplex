import { describe, expect, it } from "vitest";
import { sampleFromArrowTable } from "./profileClient";

// Minimal stand-in for an apache-arrow Table -- just enough surface for
// sampleFromArrowTable (schema.fields, getChild(name).get(row), numRows).
function makeArrowTable(columnsByName) {
  const names = Object.keys(columnsByName);
  return {
    schema: { fields: names.map((name) => ({ name })) },
    numRows: columnsByName[names[0]].length,
    getChild: (name) => ({ get: (row) => columnsByName[name][row] }),
  };
}

describe("sampleFromArrowTable's getNumericColumn cache", () => {
  it("returns the correct values", () => {
    const sample = sampleFromArrowTable(makeArrowTable({ age: [21, null, 41] }));
    expect(Array.from(sample.getNumericColumn(0))).toEqual([21, NaN, 41]);
  });

  it("returns the exact same array reference on repeat calls for the same column", () => {
    const sample = sampleFromArrowTable(makeArrowTable({ age: [21, 29, 41] }));
    expect(sample.getNumericColumn(0)).toBe(sample.getNumericColumn(0));
  });

  it("caches per column independently", () => {
    const sample = sampleFromArrowTable(makeArrowTable({ age: [21, 29], score: [1, 2] }));
    const age = sample.getNumericColumn(0);
    const score = sample.getNumericColumn(1);

    expect(age).not.toBe(score);
    expect(Array.from(age)).toEqual([21, 29]);
    expect(Array.from(score)).toEqual([1, 2]);
  });
});
