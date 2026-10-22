// Opening a recipe file: which dropped or chosen file is a recipe, and how a
// recipe (already parsed and checked by Rust, `recipe_yaml_to_json`) maps onto
// the loaded table. Plain functions over plain objects; the hook that applies
// the result is `useOpenRecipe`.
import { isKeyedMethod, RECIPE_SALT_ENV } from "./yamlBuilder";

export const RECIPE_FILE_EXTENSIONS = ["yaml", "yml"];

/** Whether a file name looks like a recipe (`.triplex.yaml`, `.yaml`, `.yml`). */
export function isRecipeFileName(name) {
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  return name.includes(".") && RECIPE_FILE_EXTENSIONS.includes(ext);
}

/** One opened recipe column in the shape the page builds recipes in
 * (`upsertRecipeColumn`): name, method and `salt_env` (empty for a method
 * without a salt), plus the settings only this method reads. The settings are
 * kept so a recipe opened and downloaded again comes out unchanged. */
function toPageColumn(column) {
  const out = {
    name: column.name,
    method: column.method,
    salt_env: isKeyedMethod(column.method) ? column.salt_env : "",
  };
  if (column.method === "bucket") out.bucket_width = column.bucket_width;
  if (column.method === "partial_mask") {
    out.keep_prefix = column.keep_prefix;
    out.keep_suffix = column.keep_suffix;
  }
  return out;
}

/**
 * Maps an opened recipe onto the table's columns. The recipe replaces the
 * current one: table columns it does not name end up with no method, and the
 * columns it drops are exactly the ones hidden from the table.
 *
 * @param {{ version: number, columns: any[] }} recipe parsed recipe JSON
 * @param {string[]} headers the loaded table's column names
 * @returns {{
 *   recipe: { version: number, columns: any[] },
 *   removedColumns: Set<string>,
 *   missing: string[],
 *   keyedColumns: string[],
 *   saltEnvs: string[],
 * }} `missing` lists recipe columns the table does not have (not applied);
 *   `keyedColumns` the applied columns that need the salt; `saltEnvs` the
 *   distinct salt variables those columns name.
 */
export function planRecipeOpen(recipe, headers) {
  const inTable = new Set(headers);
  const applied = recipe.columns.filter((c) => inTable.has(c.name)).map(toPageColumn);
  const keyed = applied.filter((c) => isKeyedMethod(c.method));
  return {
    recipe: { version: recipe.version, columns: applied },
    removedColumns: new Set(applied.filter((c) => c.method === "drop").map((c) => c.name)),
    missing: recipe.columns.filter((c) => !inTable.has(c.name)).map((c) => c.name),
    keyedColumns: keyed.map((c) => c.name),
    saltEnvs: [...new Set(keyed.map((c) => c.salt_env))],
  };
}

/** The settings an opened recipe gave a column that the column menu has no control for, as
 * text, or null when the column uses the defaults the menu would pick (a method picked by hand
 * carries no settings at all). */
export function recipeSettingsText(column) {
  if (!column) return null;
  const parts = [];
  if (column.method === "bucket" && column.bucket_width !== undefined) {
    parts.push(`bucket width ${column.bucket_width}`);
  }
  if (column.method === "partial_mask" && column.keep_prefix !== undefined) {
    parts.push(
      `keeps the first ${column.keep_prefix} and the last ${column.keep_suffix} characters`,
    );
  }
  if (isKeyedMethod(column.method) && column.salt_env && column.salt_env !== RECIPE_SALT_ENV) {
    parts.push(`salt variable ${column.salt_env} for the command-line tool`);
  }
  return parts.length > 0 ? parts.join(", ") : null;
}

/** A list of names for a message: `"a", "b" and "c"`. */
export function listNames(names) {
  const quoted = names.map((n) => `"${n}"`);
  if (quoted.length <= 1) return quoted.join("");
  return `${quoted.slice(0, -1).join(", ")} and ${quoted.at(-1)}`;
}
