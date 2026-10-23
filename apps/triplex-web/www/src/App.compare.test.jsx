import { describe, it, expect, vi, afterEach, beforeAll } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { EXTENSION_API_VERSION } from "./extensionApi";

// The engine is faked (testing/fakeEngine.js): jsdom runs no wasm and no Web Workers. The table,
// the side panel, the column cards and the matrices draw on a canvas, so they are stand-ins too;
// what this file tests is which two tables the Compare view gets and what it says about them.
vi.mock("./hooks/useWasm", async () => {
  const { fakeWasm } = await import("./testing/fakeEngine");
  return { useWasm: () => ({ wasm: fakeWasm, methods: [] }) };
});
vi.mock("./state/profileClient", async () => {
  const { fakeProfile } = await import("./testing/fakeEngine");
  return { profileFile: async (bytes) => fakeProfile(bytes) };
});
vi.mock("./state/anonymizeClient", async () => {
  const { fakeAnonymize } = await import("./testing/fakeEngine");
  return {
    runAnonymize: async (...args) => fakeAnonymize(...args),
    checkPatternConformance: async () => "{}",
    convertBytes: async (bytes) => bytes,
  };
});
vi.mock("./state/datasetCardClient", () => ({
  scoreColumnRisk: async () => ({}),
  scoreDiversityMatrix: async () => ({}),
}));
vi.mock("./hooks/useDatasetAnalysis", () => ({
  useDatasetAnalysis: () => ({
    columnRisk: { riskByName: {}, riskLoading: false },
    diversityMatrix: { raw: null, loading: false, error: null, quasiColumns: [], allColumns: [] },
    rowFilter: { filteredRowIndices: null, setRowFilter: () => {} },
    columnSubsets: {},
    association: null,
  }),
}));
// Stands in for the table: its Anonymize action, and the notice's way into Compare once App
// offers one.
vi.mock("./components/TableTab", () => ({
  default: ({ onAnonymize, onCompareAnonymized }) => (
    <div>
      <button type="button" onClick={onAnonymize}>
        Run anonymize
      </button>
      {onCompareAnonymized && (
        <button type="button" onClick={onCompareAnonymized}>
          Compare with the original
        </button>
      )}
    </div>
  ),
}));
vi.mock("./components/AnalysisOverlay", () => ({ default: () => null }));
vi.mock("./components/ColumnCard", () => ({ default: ({ col }) => <div>card {col.name}</div> }));
// Shows which quasi-identifiers each side's matrices were computed on.
vi.mock("./components/DiversityCheckSection", () => ({
  DiversityCompare: ({ left, right }) => (
    <p>
      matrices: {left.quasiColumns.map((c) => c.name).join(",")} |{" "}
      {right.quasiColumns.map((c) => c.name).join(",")}
    </p>
  ),
}));

const { default: App } = await import("./App");

afterEach(cleanup);
// jsdom has <dialog> but not its modal methods.
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
    this.dispatchEvent(new Event("close"));
  };
});

const PEOPLE = `id,name,age,sex,city
1,Ann,31,F,Utrecht
2,Bob,35,M,Utrecht
3,Cas,42,M,Zeist
4,Dee,47,F,Zeist
5,Eva,33,F,Utrecht
6,Fay,38,F,Utrecht
`;
const RECIPE = {
  version: 1,
  columns: [
    { name: "id", method: "token_hmac", salt_env: "ID_SALT" },
    { name: "name", method: "drop" },
    { name: "age", method: "bucket", bucket_width: 10 },
  ],
};
const recipeFile = (recipe) =>
  new File([JSON.stringify(recipe)], "recipe.triplex.yaml", { type: "text/yaml" });

const compareButton = () => screen.getByRole("button", { name: "Compare datasets" });

async function renderLoaded(props = {}) {
  const view = render(
    <App extensions={[]} initialFile={new File([PEOPLE], "people.csv")} {...props} />,
  );
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Open recipe" }).hasAttribute("disabled")).toBe(
      false,
    ),
  );
  return view;
}

async function openRecipe(container, recipe, replace = false) {
  const input = container.querySelector('input[type="file"][accept=".yaml,.yml"]');
  fireEvent.change(input, { target: { files: [recipeFile(recipe)] } });
  if (replace) fireEvent.click(await screen.findByRole("button", { name: "Replace recipe" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Download recipe" }).hasAttribute("disabled")).toBe(
      false,
    ),
  );
}

async function anonymize() {
  fireEvent.click(screen.getByRole("button", { name: "Run anonymize" }));
  await waitFor(() => expect(compareButton().hasAttribute("disabled")).toBe(false));
}

const dialog = () => screen.getByRole("dialog", { name: "Compare datasets" });
// The dialog is code-split, so it appears a moment after Compare is clicked.
const openedDialog = () => screen.findByRole("dialog", { name: "Compare datasets" });
const gaps = () => [...dialog().querySelectorAll(".compare-gap")].map((el) => el.textContent);

describe("Compare: original and anonymized", () => {
  it("is offered without any extension once the data is anonymized", async () => {
    const { container } = await renderLoaded();
    expect(compareButton().hasAttribute("disabled")).toBe(true);
    expect(compareButton().title).toBe("Anonymize the data first");
    expect(screen.queryByRole("button", { name: "Compare with the original" })).toBeNull();

    await openRecipe(container, RECIPE);
    await anonymize();
    // A dot next to the button says there is something new to compare, until it is opened.
    expect(compareButton().querySelector(".pulse-dot")).toBeTruthy();
    fireEvent.click(compareButton());
    await openedDialog();
    expect(compareButton().querySelector(".pulse-dot")).toBeNull();

    expect(
      within(dialog()).getByRole("heading", { name: "Compare: Original and Anonymized" }),
    ).toBeTruthy();
    expect(within(dialog()).getAllByText("Original").length).toBeGreaterThan(0);
    expect(within(dialog()).getAllByText("Anonymized").length).toBeGreaterThan(0);
    // One thing to compare with, so no switch.
    expect(within(dialog()).queryByRole("group", { name: "Compare the original with" })).toBeNull();
  });

  it("shows a dropped column as removed, with the reason", async () => {
    const { container } = await renderLoaded();
    await openRecipe(container, RECIPE);
    await anonymize();
    fireEvent.click(screen.getByRole("button", { name: "Compare with the original" }));
    await openedDialog();
    // The original side keeps the column the recipe drops, so the pair shows what happened to it.
    expect(within(dialog()).getByText("card name")).toBeTruthy();
    expect(gaps()).toEqual(["name is not in the anonymized dataset: the recipe drops it."]);
  });

  it("measures both sides on the same quasi-identifiers", async () => {
    const { container } = await renderLoaded();
    await openRecipe(container, RECIPE);
    await anonymize();
    fireEvent.click(compareButton());
    await openedDialog();
    fireEvent.click(within(dialog()).getByRole("tab", { name: "Privacy matrices" }));

    const chips = within(dialog()).getByRole("group", { name: "Quasi-identifiers" });
    const pressed = (name) =>
      within(chips).getByRole("button", { name }).getAttribute("aria-pressed");
    // id is a pseudonym on the anonymized side, one value per person: offered, not chosen.
    expect(
      within(chips)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["id", "age", "sex", "city"]);
    expect(pressed("id")).toBe("false");
    expect(within(dialog()).getByText("matrices: age,sex,city | age,sex,city")).toBeTruthy();

    // Every age is different before; in ranges of 10, three people share 30-39, F, Utrecht.
    const alone = () =>
      [...dialog().querySelectorAll(".compare-alone")].map((el) => el.textContent);
    await waitFor(() =>
      expect(alone()).toEqual([
        "6 of 6 rows (100.0%) share their combination of these values with no other row in the original table.",
        "3 of 6 rows (50.0%) share their combination of these values with no other row in the anonymized table.",
      ]),
    );

    fireEvent.click(within(chips).getByRole("button", { name: "age" }));
    expect(within(dialog()).getByText("matrices: sex,city | sex,city")).toBeTruthy();
  });

  it("says when the recipe changed after the run, and a new run replaces the result", async () => {
    const { container } = await renderLoaded();
    await openRecipe(container, RECIPE);
    await anonymize();

    const next = {
      version: 1,
      columns: [
        { name: "city", method: "drop" },
        { name: "age", method: "bucket", bucket_width: 10 },
      ],
    };
    await openRecipe(container, next, true);
    fireEvent.click(compareButton());
    await openedDialog();
    expect(within(dialog()).getByRole("note").textContent).toMatch(/^From the last run\./);
    expect(gaps()).toEqual(["name is not in the anonymized dataset: the recipe drops it."]);
    fireEvent.click(within(dialog()).getByRole("button", { name: "Close" }));

    fireEvent.click(screen.getByRole("button", { name: "Run anonymize" }));
    fireEvent.click(await screen.findByRole("button", { name: "Compare with the original" }));
    await openedDialog();
    await waitFor(() =>
      expect(gaps()).toEqual(["city is not in the anonymized dataset: the recipe drops it."]),
    );
    expect(within(dialog()).queryByRole("note")).toBeNull();
  });

  it("is cleared when another file is loaded", async () => {
    const { container } = await renderLoaded();
    await openRecipe(container, RECIPE);
    await anonymize();

    const dataInput = container.querySelector('input[type="file"]:not([accept=".yaml,.yml"])');
    fireEvent.change(dataInput, { target: { files: [new File([PEOPLE], "other.csv")] } });
    await screen.findByText(/other\.csv/);
    expect(compareButton().hasAttribute("disabled")).toBe(true);
    expect(screen.queryByRole("button", { name: "Compare with the original" })).toBeNull();
  });

  it("lets the user choose between the anonymized and a generated table", async () => {
    const made = fakeProfileOf("age,sex,city\n30,F,Utrecht\n44,M,Zeist\n");
    /** @type {import("./extensionApi").GeneratorExtension} */
    const fake = {
      apiVersion: EXTENSION_API_VERSION,
      id: "made",
      tier: /** @type {2} */ (2),
      label: "Made",
      noun: "made",
      emptyHint: "Make data first",
      useGenerator: () => ({
        result: {
          ...made,
          report: { columns_dropped: [{ column: "id", reason: "an identifier" }] },
        },
        clearResult: () => {},
        reset: () => {},
        downloadCsv: () => {},
        downloadReport: () => {},
      }),
      Dialog: () => null,
      history: { label: "Make data", detail: () => "" },
    };
    const { container } = await renderLoaded({ extensions: [fake] });
    expect(compareButton().hasAttribute("disabled")).toBe(false);
    await openRecipe(container, RECIPE);
    await anonymize();

    fireEvent.click(compareButton());
    await openedDialog();
    const choices = within(dialog()).getByRole("group", { name: "Compare the original with" });
    const choice = (name) => within(choices).getByRole("button", { name });
    expect(choice("Anonymized").getAttribute("aria-pressed")).toBe("true");

    fireEvent.click(choice("Made"));
    expect(
      within(dialog()).getByRole("heading", { name: "Compare: Original and Made" }),
    ).toBeTruthy();
    expect(gaps()).toContain("id is not in the made dataset: an identifier.");
  });
});

function fakeProfileOf(csv) {
  const headers = csv.trim().split("\n")[0].split(",");
  const rows = csv
    .trim()
    .split("\n")
    .slice(1)
    .map((l) => l.split(","));
  return {
    sample: {
      headers,
      rowCount: rows.length,
      getCell: (r, c) => rows[r][c],
      getNumericColumn: (c) => Float64Array.from(rows, (r) => Number(r[c])),
    },
    profile: {
      columns: headers.map((name) => ({
        name,
        inferred_type: "string",
        null_percentage: 0,
        distribution: { kind: "categorical", cardinality: rows.length, top_values: [] },
      })),
    },
  };
}
