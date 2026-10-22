import { listNames } from "../state/recipeFile";

/** What is left to do after a recipe was opened: the columns it named that the table does not
 * have (not applied), and the keyed columns, which need the salt typed in because a recipe never
 * contains it. The salt part goes away once a salt is typed; the rest stays until closed. */
export default function RecipeNotice({ opened, saltTyped, onDismiss }) {
  const { fileName, missing, keyedColumns, saltEnvs } = opened;
  const needsSalt = keyedColumns.length > 0 && !saltTyped;
  if (missing.length === 0 && !needsSalt) return null;
  return (
    <div className="banner banner-warning recipe-notice" role="status">
      <div>
        {missing.length > 0 && (
          <p>
            Not applied from {fileName}, not in this table: {listNames(missing)}.
          </p>
        )}
        {needsSalt && (
          <p>
            {listNames(keyedColumns)} {keyedColumns.length === 1 ? "uses" : "use"} a keyed method.
            Type the salt in the column menu before you anonymize: a recipe never contains it.
          </p>
        )}
        {needsSalt && saltEnvs.length > 1 && (
          <p>
            The recipe names {saltEnvs.length} salt variables ({saltEnvs.join(", ")}). This page
            uses one salt for all of them, so its tokens match the command-line tool&apos;s only
            when those variables hold that same salt.
          </p>
        )}
      </div>
      <button type="button" onClick={onDismiss} aria-label="Close">
        Close
      </button>
    </div>
  );
}
