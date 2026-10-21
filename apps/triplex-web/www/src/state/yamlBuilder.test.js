import { describe, it, expect } from "vitest";
import { createEmptyRecipe, saltEnvFor, upsertRecipeColumn, RECIPE_SALT_ENV } from "./yamlBuilder";

describe("recipe salt variable", () => {
  it("names the salt variable only for keyed methods", () => {
    expect(saltEnvFor("token_hmac")).toBe(RECIPE_SALT_ENV);
    expect(saltEnvFor("fpe")).toBe(RECIPE_SALT_ENV);
    for (const m of ["bucket", "partial_mask", "drop"]) expect(saltEnvFor(m)).toBe("");
  });

  it("writes it into the recipe column", () => {
    const recipe = upsertRecipeColumn(
      createEmptyRecipe(),
      "email",
      "token_hmac",
      saltEnvFor("token_hmac"),
    );
    expect(recipe.columns).toEqual([
      { name: "email", method: "token_hmac", salt_env: "TRIPLEX_SALT" },
    ]);
  });
});
