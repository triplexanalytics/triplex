import { describe, it, expect } from "vitest";
import { isRecipeFileName, listNames, planRecipeOpen, recipeSettingsText } from "./recipeFile";

// What `recipe_yaml_to_json` returns: each column carries only the settings its method uses.
const OPENED = {
  version: 1,
  columns: [
    { name: "email", method: "token_hmac", salt_env: "CUSTOMER_SALT" },
    { name: "iban", method: "fpe", salt_env: "TRIPLEX_SALT" },
    { name: "age", method: "bucket", bucket_width: 5 },
    { name: "card", method: "partial_mask", keep_prefix: 2, keep_suffix: 3 },
    { name: "notes", method: "drop" },
  ],
};
const HEADERS = ["id", "email", "iban", "age", "card", "notes"];

describe("isRecipeFileName", () => {
  it("takes .yaml and .yml files, in any case, and nothing else", () => {
    expect(isRecipeFileName("recipe.triplex.yaml")).toBe(true);
    expect(isRecipeFileName("RECIPE.YML")).toBe(true);
    expect(isRecipeFileName("people.csv")).toBe(false);
    expect(isRecipeFileName("yaml")).toBe(false);
  });
});

describe("planRecipeOpen", () => {
  it("sets every named column's method and settings as the page builds them", () => {
    const plan = planRecipeOpen(OPENED, HEADERS);
    expect(plan.recipe).toEqual({
      version: 1,
      columns: [
        { name: "email", method: "token_hmac", salt_env: "CUSTOMER_SALT" },
        { name: "iban", method: "fpe", salt_env: "TRIPLEX_SALT" },
        { name: "age", method: "bucket", salt_env: "", bucket_width: 5 },
        { name: "card", method: "partial_mask", salt_env: "", keep_prefix: 2, keep_suffix: 3 },
        { name: "notes", method: "drop", salt_env: "" },
      ],
    });
    expect(plan.missing).toEqual([]);
  });

  it("hides exactly the dropped columns", () => {
    expect([...planRecipeOpen(OPENED, HEADERS).removedColumns]).toEqual(["notes"]);
  });

  it("lists the recipe columns the table does not have and leaves them out", () => {
    const plan = planRecipeOpen(OPENED, ["id", "age", "notes"]);
    expect(plan.missing).toEqual(["email", "iban", "card"]);
    expect(plan.recipe.columns.map((c) => c.name)).toEqual(["age", "notes"]);
    expect(plan.keyedColumns).toEqual([]);
  });

  it("names the keyed columns and the salt variables they read", () => {
    const plan = planRecipeOpen(OPENED, HEADERS);
    expect(plan.keyedColumns).toEqual(["email", "iban"]);
    expect(plan.saltEnvs).toEqual(["CUSTOMER_SALT", "TRIPLEX_SALT"]);
  });

  it("applies nothing when no column matches", () => {
    const plan = planRecipeOpen(OPENED, ["id"]);
    expect(plan.recipe.columns).toEqual([]);
    expect(plan.missing).toHaveLength(5);
  });
});

describe("recipeSettingsText", () => {
  it("describes settings the column menu has no control for", () => {
    const [email, iban, age, card, notes] = planRecipeOpen(OPENED, HEADERS).recipe.columns;
    expect(recipeSettingsText(age)).toBe("bucket width 5");
    expect(recipeSettingsText(card)).toBe("keeps the first 2 and the last 3 characters");
    expect(recipeSettingsText(email)).toBe("salt variable CUSTOMER_SALT for the command-line tool");
    expect(recipeSettingsText(iban)).toBeNull();
    expect(recipeSettingsText(notes)).toBeNull();
  });

  it("says nothing for a method picked by hand", () => {
    expect(recipeSettingsText({ name: "age", method: "bucket", salt_env: "" })).toBeNull();
    expect(recipeSettingsText(undefined)).toBeNull();
  });
});

describe("listNames", () => {
  it("quotes and joins names for a sentence", () => {
    expect(listNames(["a"])).toBe('"a"');
    expect(listNames(["a", "b"])).toBe('"a" and "b"');
    expect(listNames(["a", "b", "c"])).toBe('"a", "b" and "c"');
  });
});
