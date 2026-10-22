import ConfirmDialog from "./ConfirmDialog";
import { listNames } from "../state/recipeFile";

const columns = (n) => `${n} ${n === 1 ? "column" : "columns"}`;

/** Asks before an opened recipe is applied when that would replace a recipe already started,
 * or when some of its columns are not in the table (the others are applied only on a yes). */
export default function OpenRecipeDialog({ pending, onConfirm, onCancel }) {
  const { fileName, plan, replacing } = pending;
  const applied = plan.recipe.columns.length;
  const missing = plan.missing;
  return (
    <ConfirmDialog
      title="Open recipe"
      confirmLabel={missing.length > 0 ? `Apply ${columns(applied)}` : "Replace recipe"}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      {missing.length > 0 && (
        <p>
          {missing.length} of {missing.length + applied} columns in {fileName}{" "}
          {missing.length === 1 ? "is" : "are"} not in this table: {listNames(missing)}. Apply the
          other {columns(applied)}?
        </p>
      )}
      {replacing > 0 && (
        <p>
          This replaces the current recipe, which sets a method on {columns(replacing)}. You can
          undo it in the History tab.
        </p>
      )}
    </ConfirmDialog>
  );
}
