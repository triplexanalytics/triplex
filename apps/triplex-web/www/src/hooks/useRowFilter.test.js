import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useRowFilter } from "./useRowFilter";

// Two columns, 4 rows: "city" (string, exact-match bucketing) and "age"
// (integer, wasm-bucketed) -- enough to exercise every rowFilter shape
// useRowFilter resolves without a real wasm module.
const CITY = ["NYC", "NYC", "LA", "LA"];
const AGE = [21, 29, 41, 49];

function makeSample() {
  return {
    headers: ["city", "age"],
    rowCount: 4,
    getCell: (row, colIndex) => (colIndex === 0 ? CITY[row] : AGE[row]),
    getNumericColumn: (colIndex) => {
      if (colIndex !== 1) throw new Error("only 'age' is numeric in this fixture");
      return Float64Array.from(AGE);
    },
  };
}

function makeProfile() {
  return {
    columns: [
      { name: "city", inferred_type: "string" },
      {
        name: "age",
        inferred_type: "integer",
        distribution: {
          // Two 10-wide bins covering the fixture's age range, matching
          // numericBinIndex's own (value - lower) / (upper - lower) math.
          bins: [
            { lower: 20, upper: 30 },
            { lower: 40, upper: 50 },
          ],
        },
      },
    ],
  };
}

// Buckets every age into its decade (21->20, 29->20, 41->40, 49->40) --
// only asserted shape useRowFilter relies on from the real wasm export.
function makeWasm() {
  return {
    bucket_numeric_column: (values, width) =>
      Float64Array.from(values, (v) => Math.floor(v / width) * width),
  };
}

describe("useRowFilter", () => {
  it("filteredRowIndices is null when no filter is set", () => {
    const { result } = renderHook(() => useRowFilter(makeWasm(), makeSample(), makeProfile()));
    expect(result.current.filteredRowIndices).toBeNull();
  });

  it("bucketValue on a string column matches exact cell values", () => {
    const { result } = renderHook(() => useRowFilter(makeWasm(), makeSample(), makeProfile()));
    act(() => result.current.setRowFilter([{ columnName: "city", bucketValue: "LA" }]));

    expect(result.current.filteredRowIndices).toEqual(new Set([2, 3]));
  });

  it("bucketValue on an integer column matches via wasm's decade bucketing", () => {
    const { result } = renderHook(() => useRowFilter(makeWasm(), makeSample(), makeProfile()));
    act(() => result.current.setRowFilter([{ columnName: "age", bucketValue: 20 }]));

    expect(result.current.filteredRowIndices).toEqual(new Set([0, 1]));
  });

  it("binIndexFrom/binIndexTo resolves via the column's own distribution bins", () => {
    const { result } = renderHook(() => useRowFilter(makeWasm(), makeSample(), makeProfile()));
    // Both fixture bins (index 0 and 1) -> every row.
    act(() => result.current.setRowFilter([{ columnName: "age", binIndexFrom: 0, binIndexTo: 1 }]));

    expect(result.current.filteredRowIndices).toEqual(new Set([0, 1, 2, 3]));
  });

  it("bucketValues matches rows whose exact cell value is in the set", () => {
    const { result } = renderHook(() => useRowFilter(makeWasm(), makeSample(), makeProfile()));
    act(() => result.current.setRowFilter([{ columnName: "city", bucketValues: ["NYC"] }]));

    expect(result.current.filteredRowIndices).toEqual(new Set([0, 1]));
  });

  it("ANDs multiple filter conditions together", () => {
    const { result } = renderHook(() => useRowFilter(makeWasm(), makeSample(), makeProfile()));
    act(() =>
      result.current.setRowFilter([
        { columnName: "city", bucketValue: "LA" },
        { columnName: "age", bucketValue: 40 },
      ]),
    );

    expect(result.current.filteredRowIndices).toEqual(new Set([2, 3]));
  });

  it("clearing the filter (setRowFilter(null)) goes back to null", () => {
    const { result } = renderHook(() => useRowFilter(makeWasm(), makeSample(), makeProfile()));
    act(() => result.current.setRowFilter([{ columnName: "city", bucketValue: "LA" }]));
    act(() => result.current.setRowFilter(null));

    expect(result.current.filteredRowIndices).toBeNull();
  });
});
