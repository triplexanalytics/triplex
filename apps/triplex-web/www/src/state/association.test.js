import { describe, expect, it } from "vitest";
import { computeAssociation, correlationRatio, cramersV, pearson } from "./association";

function fakeSample(columns) {
  const headers = Object.keys(columns);
  const rowCount = columns[headers[0]].length;
  return {
    headers,
    rowCount,
    getCell: (row, c) => String(columns[headers[c]][row] ?? ""),
    getNumericColumn: (c) =>
      Float64Array.from(columns[headers[c]], (v) => (v === null ? NaN : Number(v))),
  };
}
const num = (name) => ({ name, distribution: { kind: "numeric" } });
const cat = (name) => ({ name, distribution: { kind: "categorical" } });

describe("pearson", () => {
  it("is 1 and -1 for perfectly linear pairs", () => {
    expect(pearson([1, 2, 3, 4], [2, 4, 6, 8])).toBeCloseTo(1, 10);
    expect(pearson([1, 2, 3, 4], [8, 6, 4, 2])).toBeCloseTo(-1, 10);
  });
  it("skips pairs with a missing value and is null for a constant column", () => {
    expect(pearson([1, NaN, 3, 4], [2, 9, 6, 8])).toBeCloseTo(1, 10);
    expect(pearson([1, 1, 1], [1, 2, 3])).toBeNull();
  });
});

describe("cramersV", () => {
  const toCol = (codes) => ({ kind: "categorical", codes: Int32Array.from(codes), levels: 2 });
  it("is near 1 when one category determines the other", () => {
    const a = toCol([0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1]);
    expect(cramersV(a, a)).toBeGreaterThan(0.9);
  });
  it("is 0 for independent categories", () => {
    const a = toCol([0, 0, 1, 1, 0, 0, 1, 1]);
    const b = toCol([0, 1, 0, 1, 0, 1, 0, 1]);
    expect(cramersV(a, b)).toBeCloseTo(0, 10);
  });
});

describe("correlationRatio", () => {
  const c = { kind: "categorical", codes: Int32Array.from([0, 0, 1, 1]), levels: 2 };
  it("is 1 when the category fully explains the number", () => {
    expect(
      correlationRatio(c, { kind: "numeric", values: Float64Array.from([1, 1, 5, 5]) }),
    ).toBeCloseTo(1, 10);
  });
  it("is 0 when group means are equal", () => {
    expect(
      correlationRatio(c, { kind: "numeric", values: Float64Array.from([1, 5, 1, 5]) }),
    ).toBeCloseTo(0, 10);
  });
});

describe("computeAssociation", () => {
  const sample = fakeSample({
    id: [1, 2, 3, 4, 5, 6],
    x: [1, 2, 3, 4, 5, 6],
    y: [2, 4, 6, 8, 10, 12],
    plan: ["a", "a", "a", "b", "b", "b"],
  });
  const profile = {
    columns: [
      { ...num("id"), semantic_type: "likely_identifier" },
      num("x"),
      num("y"),
      cat("plan"),
    ],
  };

  it("leaves identifiers out and picks the measure by column kinds", () => {
    const a = computeAssociation(sample, profile);
    expect(a.names).toEqual(["x", "y", "plan"]);
    expect(a.cells[0][1]).toMatchObject({ measure: "pearson" });
    expect(a.cells[0][1].signed).toBeCloseTo(1, 10);
    expect(a.cells[0][2].measure).toBe("eta");
    expect(a.cells[0][0]).toBeNull();
  });

  it("is symmetric", () => {
    const a = computeAssociation(sample, profile);
    expect(a.cells[1][0].value).toBeCloseTo(a.cells[0][1].value, 12);
    expect(a.cells[2][0].value).toBeCloseTo(a.cells[0][2].value, 12);
  });

  it("leaves out a category column with too many distinct values", () => {
    const n = 120;
    const s = fakeSample({
      x: Array.from({ length: n }, (_, i) => i),
      y: Array.from({ length: n }, (_, i) => i * 2),
      email: Array.from({ length: n }, (_, i) => `user${i}@example.com`),
    });
    const a = computeAssociation(s, { columns: [num("x"), num("y"), cat("email")] });
    expect(a.names).toEqual(["x", "y"]);
  });

  it("ignores excluded rows", () => {
    // Rows 4 and 5 break the perfect line; without them r is exactly 1 again.
    const s = fakeSample({ x: [1, 2, 3, 4, 5], y: [2, 4, 6, 100, -50] });
    const p = { columns: [num("x"), num("y")] };
    expect(computeAssociation(s, p).cells[0][1].value).toBeLessThan(0.9);
    expect(computeAssociation(s, p, new Set([3, 4])).cells[0][1].value).toBeCloseTo(1, 10);
  });
});
