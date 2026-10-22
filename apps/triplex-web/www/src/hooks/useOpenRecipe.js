import { useState } from "react";
import { listNames, planRecipeOpen } from "../state/recipeFile";

const columnCount = (n) => `${n} ${n === 1 ? "col" : "cols"}`;

/** "Open recipe": reads a recipe file, has Rust parse and check it
 * (`recipe_yaml_to_json`, the same rules as the command-line tool), maps it
 * onto the loaded table and applies it as if each method had been picked by
 * hand: one History entry, undoable as a whole.
 *
 * Nothing is applied silently in part. A recipe that does not parse, or names
 * no column of this table, is an `error` and changes nothing. A recipe that
 * names columns the table lacks, or would replace a recipe already started,
 * waits in `pending` until the user confirms or cancels.
 *
 * After opening, `opened` says what the user still has to know: columns that
 * were not applied, and the keyed columns, whose salt is never in a recipe.
 * `saltRequest` asks the table to open the first keyed column's menu with the
 * cursor in the salt field; the table calls `clearSaltRequest` once it has. */
export function useOpenRecipe({ wasm, file, history }) {
  const [error, setError] = useState("");
  const [pending, setPending] = useState(null);
  const [opened, setOpened] = useState(null);
  const [saltRequest, setSaltRequest] = useState(null);

  const apply = ({ fileName, plan }) => {
    const prevRecipe = file.recipe;
    const prevRemovedColumns = file.removedColumns;
    file.setRecipe(plan.recipe);
    file.setRemovedColumns(plan.removedColumns);
    history.log("Open recipe", columnCount(plan.recipe.columns.length), null, () => {
      file.setRecipe(prevRecipe);
      file.setRemovedColumns(prevRemovedColumns);
      setOpened(null);
    });
    setOpened({
      fileName,
      missing: plan.missing,
      keyedColumns: plan.keyedColumns,
      saltEnvs: plan.saltEnvs,
    });
    if (plan.keyedColumns.length > 0 && !file.salt) {
      setSaltRequest((r) => ({ name: plan.keyedColumns[0], seq: (r?.seq ?? 0) + 1 }));
    }
  };

  const open = async (recipeFile) => {
    setError("");
    setOpened(null);
    if (!file.sample) {
      setError("Load a data file first, then open the recipe.");
      return;
    }
    if (!wasm) return;
    let parsed;
    try {
      parsed = JSON.parse(wasm.recipe_yaml_to_json(await recipeFile.text()));
    } catch (err) {
      setError(`${recipeFile.name} could not be opened. ${String(err?.message || err)}`);
      return;
    }
    const plan = planRecipeOpen(parsed, file.sample.headers);
    if (plan.recipe.columns.length === 0) {
      setError(
        `None of the columns in ${recipeFile.name} is in this table: ${listNames(plan.missing)}. ` +
          "Nothing was changed.",
      );
      return;
    }
    const opening = { fileName: recipeFile.name, plan, replacing: file.recipe.columns.length };
    if (plan.missing.length > 0 || opening.replacing > 0) setPending(opening);
    else apply(opening);
  };

  const confirm = () => {
    if (pending) apply(pending);
    setPending(null);
  };

  // A new data file starts over: nothing from the last one's recipe applies to it.
  const reset = () => {
    setError("");
    setPending(null);
    setOpened(null);
    setSaltRequest(null);
  };

  return {
    open,
    pending,
    confirm,
    cancel: () => setPending(null),
    error,
    setError,
    opened,
    dismissOpened: () => setOpened(null),
    saltRequest,
    clearSaltRequest: () => setSaltRequest(null),
    reset,
  };
}
