import { describe, expect, it } from "vitest";
import {
  axisExtent,
  dropClass,
  setAxisRange,
  setClassSize,
  sketchProblem,
  validateSketchSettings,
} from "./sketchEdit";

const sketch = () => ({
  version: 1,
  columns: [
    { name: "x", type: "numeric", integer: false },
    { name: "kind", type: "categorical", values: ["a", "b"] },
  ],
  classes: [
    {
      name: "Class 1",
      size: 300,
      axes: [
        { type: "numeric", min: 0, max: 10, density: [1, 1], null_share: 0 },
        { type: "categorical", shares: [1, 0], null_share: 0 },
      ],
    },
    {
      name: "Class 2",
      size: 100,
      axes: [
        { type: "numeric", min: 100, max: 110, density: [1, 1], null_share: 0 },
        { type: "categorical", shares: [0, 1], null_share: 0 },
      ],
    },
  ],
});

describe("proposal settings", () => {
  const ok = { classes: "4", minClassSize: "20", bins: "16" };
  it("accepts the prefilled values", () => {
    expect(validateSketchSettings(ok)).toEqual([]);
  });
  it("the minimum class size is the user's: any whole number from 1", () => {
    expect(validateSketchSettings({ ...ok, minClassSize: "1" })).toEqual([]);
    expect(validateSketchSettings({ ...ok, minClassSize: "500" })).toEqual([]);
    for (const bad of ["0", "", "2.5", "abc", "-3"]) {
      expect(validateSketchSettings({ ...ok, minClassSize: bad })[0].field).toBe("minClassSize");
    }
  });
  it("checks the number of classes and bins", () => {
    expect(validateSketchSettings({ ...ok, classes: "0" })[0].field).toBe("classes");
    expect(validateSketchSettings({ ...ok, classes: "13" })[0].field).toBe("classes");
    expect(validateSketchSettings({ ...ok, bins: "0" })[0].field).toBe("bins");
  });
});

describe("steering edits", () => {
  it("moves one end of a range and leaves everything else alone", () => {
    const before = sketch();
    const after = setAxisRange(before, 0, 0, { max: 4 });
    expect(after.classes[0].axes[0]).toMatchObject({ min: 0, max: 4, density: [1, 1] });
    expect(after.classes[1]).toBe(before.classes[1]);
    expect(before.classes[0].axes[0].max).toBe(10); // not mutated
  });
  it("never inverts a range: an end dragged past the other stops at it", () => {
    expect(setAxisRange(sketch(), 0, 0, { min: 50 }).classes[0].axes[0]).toMatchObject({
      min: 10,
      max: 10,
    });
    expect(setAxisRange(sketch(), 0, 0, { max: -5 }).classes[0].axes[0]).toMatchObject({
      min: 0,
      max: 0,
    });
  });
  it("ignores a non-number and a category axis", () => {
    expect(setAxisRange(sketch(), 0, 0, { min: NaN }).classes[0].axes[0].min).toBe(0);
    expect(setAxisRange(sketch(), 0, 1, { min: 3 }).classes[0].axes[1]).toEqual(
      sketch().classes[0].axes[1],
    );
  });
  it("changes a class size, refusing fractions and negatives", () => {
    expect(setClassSize(sketch(), 1, 300).classes[1].size).toBe(300);
    expect(setClassSize(sketch(), 1, "0").classes[1].size).toBe(0);
    expect(setClassSize(sketch(), 1, -1).classes[1].size).toBe(100);
    expect(setClassSize(sketch(), 1, 2.5).classes[1].size).toBe(100);
  });
  it("drops a class but never the last one", () => {
    const one = dropClass(sketch(), 0);
    expect(one.classes.map((c) => c.name)).toEqual(["Class 2"]);
    expect(dropClass(one, 0)).toBe(one);
  });
  it("says why a sketch cannot be generated from", () => {
    expect(sketchProblem(null)).toMatch(/Propose/);
    expect(sketchProblem(sketch())).toBeNull();
    expect(sketchProblem(setClassSize(setClassSize(sketch(), 0, 0), 1, 0))).toMatch(/size 0/);
  });
});

describe("axisExtent", () => {
  it("covers every class and the data behind them", () => {
    expect(axisExtent(sketch(), 0)).toEqual([0, 110]);
    expect(axisExtent(sketch(), 0, [-20, 500, NaN])).toEqual([-20, 500]);
  });
  it("never returns an empty span", () => {
    const flat = setAxisRange(setAxisRange(dropClass(sketch(), 1), 0, 0, { min: 5 }), 0, 0, {
      max: 5,
    });
    expect(axisExtent(flat, 0)).toEqual([4.5, 5.5]);
    expect(axisExtent(sketch(), 1)).toEqual([0, 1]);
  });
});
