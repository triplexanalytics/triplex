import { describe, it, expect } from "vitest";
import { rowsAlone, sharedQuasiIdentifiers } from "./sharedQuasiIdentifiers";
import { fakeAnonymize, fakeProfile, MUNICIPAL_RECIPE } from "../testing/fakeEngine";
import municipalCsv from "../../../../../tests/fixtures/demo/municipal.csv?raw";

const municipal = new TextEncoder().encode(municipalCsv);
const original = fakeProfile(municipal);
const masked = fakeProfile(
  fakeAnonymize(municipal, "csv", JSON.stringify(MUNICIPAL_RECIPE), "{}").masked_csv,
);

describe("rowsAlone", () => {
  // The same counts `triplex risk --quasi postcode,birth_year,sex --bucket-width 1` gives for the
  // original file and for the engine's masked output of this recipe.
  it("counts the rows no other row matches, before and after the municipal recipe", () => {
    const quasi = ["postcode", "birth_year", "sex"];
    expect(rowsAlone(original.sample, quasi, new Set())).toEqual({ alone: 723, total: 1200 });
    expect(rowsAlone(masked.sample, quasi, new Set())).toEqual({ alone: 9, total: 1200 });
  });

  it("leaves removed rows out of both counts", () => {
    const sample = {
      headers: ["sex"],
      rowCount: 3,
      getCell: (row) => ["F", "F", "M"][row],
    };
    expect(rowsAlone(sample, ["sex"], new Set())).toEqual({ alone: 1, total: 3 });
    expect(rowsAlone(sample, ["sex"], new Set([0]))).toEqual({ alone: 2, total: 2 });
  });
});

describe("sharedQuasiIdentifiers", () => {
  it("offers the columns both tables keep and leaves identifiers out of the default", () => {
    const { candidates, chosen } = sharedQuasiIdentifiers(original.profile, masked.profile, 1200);
    // name, email and start_date are dropped by the recipe; monthly_amount is a decimal.
    expect(candidates).toEqual([
      "client_id",
      "municipality",
      "postcode",
      "birth_year",
      "sex",
      "household",
      "scheme",
    ]);
    // client_id is a pseudonym, one value per client, so it is not chosen by default.
    expect(chosen).toEqual([
      "municipality",
      "postcode",
      "birth_year",
      "sex",
      "household",
      "scheme",
    ]);
  });

  it("leaves out a column the profile marks as an identifier", () => {
    const left = { columns: [{ name: "id", inferred_type: "integer" }] };
    const right = {
      columns: [{ name: "id", inferred_type: "integer", semantic_type: "likely_identifier" }],
    };
    expect(sharedQuasiIdentifiers(left, right, 10)).toEqual({ candidates: ["id"], chosen: [] });
  });
});
