// Builds the JSON shapes matching Rust's `DeclaredContract`/`AnonymizeRecipe`
// (see `crates/triplex-schemacast/src/contract.rs`,
// `crates/triplex-anonymizer/src/recipe.rs`) from table interactions. Rust
// renders the actual `.triplex.yaml` text (`contract_json_to_yaml`/
// `recipe_json_to_yaml`) — this module only ever produces plain JS objects.

/** Starting contract: one declared column per profiled column, no
 * statistics/pattern thresholds yet — those are added by later interactions
 * (e.g. setting a pattern on a column, not built here). */
export function buildContractFromProfile(profile) {
  return {
    version: 1,
    columns: profile.columns.map((c) => ({
      name: c.name,
      type: c.inferred_type,
      required: true,
    })),
  };
}

/** Sets (or clears, when `pattern` is null) a regex pattern assertion on one
 * declared column — the hook the Column-view tab's conformance check reads
 * from. Returns a new contract object; doesn't mutate the input. */
export function withColumnPattern(contract, columnName, pattern) {
  return {
    ...contract,
    columns: contract.columns.map((c) =>
      c.name === columnName ? { ...c, pattern: pattern || undefined } : c,
    ),
  };
}

export function createEmptyRecipe() {
  return { version: 1, columns: [] };
}

/** Adds or replaces this column's entry in the recipe — one algorithm per
 * column at a time in v1 (matches `AnonymizeColumn` having no way to declare
 * more than one method per column). */
export function upsertRecipeColumn(recipe, name, method, saltEnv) {
  const columns = recipe.columns.filter((c) => c.name !== name);
  columns.push({ name, method, salt_env: saltEnv });
  return { ...recipe, columns };
}

export function removeRecipeColumn(recipe, name) {
  return { ...recipe, columns: recipe.columns.filter((c) => c.name !== name) };
}
