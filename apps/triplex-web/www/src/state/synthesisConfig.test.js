import { describe, expect, it } from "vitest";
import {
  DP_DIAGNOSTICS_DEFAULT,
  MAX_DP_ROWS_WITH_CORRELATION,
  applySuggestions,
  blankColumns,
  buildDpConfig,
  needsPublicConfirmation,
  parseValueList,
  rowsInUse,
  updateColumn,
  validateCommon,
  validateDp,
} from "./synthesisConfig";

const suggestions = [
  {
    name: "age",
    type: "numeric",
    min: 18,
    max: 90,
    integer: true,
    suggested_include: true,
    note: null,
    derived_from_data: true,
  },
  {
    name: "plan",
    type: "categorical",
    values: ["free", "pro"],
    suggested_include: true,
    note: null,
    derived_from_data: true,
  },
  {
    name: "email",
    type: "none",
    suggested_include: false,
    note: "looks like an identifier",
    derived_from_data: true,
  },
];

const valid = (over = {}) => ({
  epsilon: "1",
  marginalShare: 0.5,
  bins: 32,
  columns: updateColumn(
    updateColumn(blankColumns(["age", "plan"]), "age", {
      include: true,
      min: "18",
      max: "90",
      integer: true,
    }),
    "plan",
    { include: true, kind: "categorical", values: "free, pro" },
  ),
  confirmedPublic: false,
  rowCount: 1000,
  ...over,
});

describe("defaults", () => {
  it("real-data diagnostics are off by default", () => {
    expect(DP_DIAGNOSTICS_DEFAULT).toBe(false);
  });
});

describe("parseValueList", () => {
  it("splits on commas and newlines, trims, drops empties and duplicates", () => {
    expect(parseValueList(" a, b ,, a\nc ")).toEqual(["a", "b", "c"]);
    expect(parseValueList("")).toEqual([]);
    expect(parseValueList(undefined)).toEqual([]);
  });
});

describe("blankColumns", () => {
  it("prefills nothing from the data and selects nothing", () => {
    const cols = blankColumns(["a", "b"]);
    expect(cols.every((c) => !c.include && c.min === "" && c.max === "" && c.values === "")).toBe(
      true,
    );
    expect(cols.every((c) => c.source === "typed")).toBe(true);
  });

  it("notes columns the user removed from the output", () => {
    const cols = blankColumns(["a", "b"], new Set(["b"]));
    expect(cols[1].note).toBe("removed from the output");
    expect(cols[0].note).toBeNull();
  });
});

describe("applySuggestions", () => {
  it("prefills domains and marks them as coming from the data", () => {
    const cols = applySuggestions(blankColumns(["age", "plan", "email"]), suggestions);
    expect(cols[0]).toMatchObject({
      kind: "numeric",
      min: "18",
      max: "90",
      integer: true,
      include: true,
      source: "suggested",
    });
    expect(cols[1]).toMatchObject({
      kind: "categorical",
      values: "free, pro",
      include: true,
      source: "suggested",
    });
  });

  it("leaves identifier-like columns unselected with their note and no domain", () => {
    const cols = applySuggestions(blankColumns(["age", "plan", "email"]), suggestions);
    expect(cols[2]).toMatchObject({
      include: false,
      min: "",
      values: "",
      note: "looks like an identifier",
    });
  });

  it("never pre-selects a column the user removed from the output", () => {
    const cols = applySuggestions(
      blankColumns(["age", "plan", "email"]),
      suggestions,
      new Set(["age"]),
    );
    expect(cols[0].include).toBe(false);
    expect(cols[0].note).toBe("removed from the output");
    expect(cols[1].include).toBe(true);
  });

  it("ignores suggestions for unknown columns", () => {
    expect(
      applySuggestions(blankColumns(["age"]), [{ ...suggestions[1], name: "other" }])[0].source,
    ).toBe("typed");
  });
});

describe("updateColumn and the public-bounds confirmation", () => {
  it("editing any part of the domain turns a suggestion into a user-declared value", () => {
    let cols = applySuggestions(blankColumns(["age"]), suggestions);
    expect(cols[0].source).toBe("suggested");
    cols = updateColumn(cols, "age", { max: "100" });
    expect(cols[0]).toMatchObject({ max: "100", source: "typed" });
  });

  it("only toggling include leaves the source as it was", () => {
    let cols = applySuggestions(blankColumns(["age"]), suggestions);
    cols = updateColumn(cols, "age", { include: false });
    expect(cols[0].source).toBe("suggested");
  });

  it("confirmation is needed while any INCLUDED column still holds a suggestion", () => {
    let cols = applySuggestions(blankColumns(["age", "plan"]), suggestions);
    expect(needsPublicConfirmation(cols)).toBe(true);
    cols = updateColumn(cols, "age", { min: "16" });
    expect(needsPublicConfirmation(cols)).toBe(true); // plan is still suggested
    cols = updateColumn(cols, "plan", { values: "free, pro, team" });
    expect(needsPublicConfirmation(cols)).toBe(false);
  });

  it("an unselected suggested column does not require confirmation", () => {
    const cols = updateColumn(applySuggestions(blankColumns(["age"]), suggestions), "age", {
      include: false,
    });
    expect(needsPublicConfirmation(cols)).toBe(false);
  });
});

describe("validateDp", () => {
  it("accepts a complete, user-declared configuration", () => {
    expect(validateDp(valid())).toEqual({ ok: true, errors: [] });
  });

  it("requires an epsilon: there is no default", () => {
    for (const epsilon of ["", "  ", "0", "-1", "abc", "NaN", "Infinity"]) {
      const r = validateDp(valid({ epsilon }));
      expect(r.ok, `epsilon ${JSON.stringify(epsilon)}`).toBe(false);
      expect(r.errors.some((e) => e.field === "epsilon")).toBe(true);
    }
    expect(validateDp(valid({ epsilon: "0.0000001" })).ok).toBe(false);
    expect(validateDp(valid({ epsilon: "0.5" })).ok).toBe(true);
  });

  it("requires at least one selected column", () => {
    const r = validateDp(valid({ columns: blankColumns(["age"]) }));
    expect(r.errors.some((e) => e.field === "columns")).toBe(true);
  });

  it("rejects numeric columns with missing or inverted bounds", () => {
    const missing = updateColumn(blankColumns(["a"]), "a", { include: true, min: "1" });
    expect(validateDp(valid({ columns: missing })).errors.some((e) => e.field === "column:a")).toBe(
      true,
    );
    const inverted = updateColumn(blankColumns(["a"]), "a", { include: true, min: "5", max: "5" });
    expect(validateDp(valid({ columns: inverted })).errors[0].message).toContain(
      "below the maximum",
    );
  });

  it("rejects categorical columns with no values or the reserved (other) value", () => {
    const none = updateColumn(blankColumns(["c"]), "c", { include: true, kind: "categorical" });
    expect(validateDp(valid({ columns: none })).ok).toBe(false);
    const reserved = updateColumn(blankColumns(["c"]), "c", {
      include: true,
      kind: "categorical",
      values: "a, (other)",
    });
    expect(validateDp(valid({ columns: reserved })).errors[0].message).toContain("reserved");
  });

  it("blocks a run while suggested bounds are unconfirmed, and allows it once confirmed", () => {
    const cols = applySuggestions(blankColumns(["age", "plan"]), suggestions);
    const blocked = validateDp(valid({ columns: cols, confirmedPublic: false }));
    expect(blocked.errors.some((e) => e.field === "confirmPublic")).toBe(true);
    expect(validateDp(valid({ columns: cols, confirmedPublic: true })).ok).toBe(true);
  });

  it("does not ask for confirmation when every bound was typed by the user", () => {
    expect(validateDp(valid({ confirmedPublic: false })).ok).toBe(true);
  });

  it("applies the backend's row cap only when two or more numeric columns are selected", () => {
    const two = updateColumn(
      updateColumn(blankColumns(["a", "b"]), "a", { include: true, min: "0", max: "1" }),
      "b",
      { include: true, min: "0", max: "1" },
    );
    expect(validateDp(valid({ columns: two, rowCount: MAX_DP_ROWS_WITH_CORRELATION })).ok).toBe(
      true,
    );
    expect(
      validateDp(valid({ columns: two, rowCount: MAX_DP_ROWS_WITH_CORRELATION + 1 })).errors.some(
        (e) => e.field === "rows",
      ),
    ).toBe(true);
    expect(validateDp(valid({ rowCount: 10 * MAX_DP_ROWS_WITH_CORRELATION })).ok).toBe(true); // one numeric column
  });

  it("checks the marginal share and the bin count", () => {
    for (const marginalShare of [0, 1, -0.2, 1.5, "x"]) {
      expect(
        validateDp(valid({ marginalShare })).errors.some((e) => e.field === "marginalShare"),
      ).toBe(true);
    }
    for (const bins of [1, 2.5, "x"]) {
      expect(validateDp(valid({ bins })).errors.some((e) => e.field === "bins")).toBe(true);
    }
  });
});

describe("buildDpConfig", () => {
  it("builds the JSON the wasm export expects, with numbers as numbers", () => {
    const config = buildDpConfig(valid({ epsilon: "2.5", marginalShare: "0.4", bins: "16" }));
    expect(config).toEqual({
      epsilon: 2.5,
      marginal_share: 0.4,
      bins: 16,
      columns: [
        { name: "age", type: "numeric", min: 18, max: 90, integer: true },
        { name: "plan", type: "categorical", values: ["free", "pro"] },
      ],
    });
  });

  it("leaves unselected columns out", () => {
    const cols = updateColumn(valid().columns, "plan", { include: false });
    expect(buildDpConfig(valid({ columns: cols })).columns.map((c) => c.name)).toEqual(["age"]);
  });
});

describe("rowsInUse", () => {
  it("subtracts removed rows and never goes negative", () => {
    expect(rowsInUse(100, 30)).toBe(70);
    expect(rowsInUse(10, 30)).toBe(0);
  });
});

describe("validateCommon", () => {
  it("accepts a whole-number seed and a blank or positive row count", () => {
    expect(validateCommon({ seed: "42", rows: "" })).toEqual([]);
    expect(validateCommon({ seed: 0, rows: "1000" })).toEqual([]);
    expect(validateCommon({ seed: "4294967295", rows: undefined })).toEqual([]);
  });

  it("rejects a missing, fractional, negative or too-large seed", () => {
    for (const seed of ["", " ", "1.5", "-1", "abc", "4294967296"]) {
      expect(
        validateCommon({ seed, rows: "" }).some((e) => e.field === "seed"),
        JSON.stringify(seed),
      ).toBe(true);
    }
  });

  it("rejects a zero, fractional or non-numeric output row count", () => {
    for (const rows of ["0", "2.5", "abc", "-4"]) {
      expect(
        validateCommon({ seed: "1", rows }).some((e) => e.field === "rowsOut"),
        rows,
      ).toBe(true);
    }
  });
});
