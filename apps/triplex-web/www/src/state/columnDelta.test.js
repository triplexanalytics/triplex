import { describe, expect, it } from "vitest";
import { distributionDifference, ksDistance, totalVariation } from "./columnDelta";

const sampleOf = (name, values) => ({
  headers: [name],
  rowCount: values.length,
  getCell: (r) => String(values[r] ?? ""),
  getNumericColumn: () => Float64Array.from(values, (v) => (v === null ? NaN : Number(v))),
});

describe("ksDistance", () => {
  it("is 0 for identical samples and 1 for disjoint ones", () => {
    expect(ksDistance(Float64Array.of(1, 2, 3), Float64Array.of(1, 2, 3))).toBe(0);
    expect(ksDistance(Float64Array.of(1, 2), Float64Array.of(10, 11))).toBe(1);
  });
  it("handles ties without overcounting", () => {
    expect(ksDistance(Float64Array.of(1, 1, 2, 2), Float64Array.of(1, 2, 2, 2))).toBeCloseTo(
      0.25,
      12,
    );
  });
});

describe("totalVariation", () => {
  const counts = (o) => ({
    counts: new Map(Object.entries(o)),
    total: Object.values(o).reduce((a, b) => a + b),
  });
  it("is 0 for equal mixes and 1 for disjoint categories", () => {
    expect(totalVariation(counts({ a: 1, b: 1 }), counts({ a: 5, b: 5 }))).toBe(0);
    expect(totalVariation(counts({ a: 2 }), counts({ b: 2 }))).toBe(1);
  });
});

describe("distributionDifference", () => {
  it("compares a column by name, ignores nulls and excluded rows, and refuses mixed kinds", () => {
    const a = sampleOf("x", [1, 2, 3, null, 99]);
    const b = sampleOf("x", [1, 2, 3]);
    expect(
      distributionDifference(
        "x",
        { sample: a, kind: "numeric", excluded: new Set([4]) },
        { sample: b, kind: "numeric" },
      ),
    ).toBe(0);
    expect(
      distributionDifference(
        "x",
        { sample: a, kind: "numeric" },
        { sample: b, kind: "categorical" },
      ),
    ).toBeNull();
    expect(
      distributionDifference(
        "missing",
        { sample: a, kind: "numeric" },
        { sample: b, kind: "numeric" },
      ),
    ).toBeNull();
  });
});
