import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

// Stands in for the engine's two recipe exports, since jsdom runs no wasm. Its "YAML" is JSON
// text; like the engine, it fills in a method's default settings when reading and writes each
// column with only the settings its method uses. The Rust tests pin that shape and check that
// real YAML comes back byte for byte; this file checks the page keeps every setting in between.
const engine = vi.hoisted(() => {
  const KEYED = ["token_hmac", "fpe"];
  const SETTINGS = { bucket: ["bucket_width"], partial_mask: ["keep_prefix", "keep_suffix"] };
  const DEFAULTS = { bucket_width: 10, keep_prefix: 0, keep_suffix: 4 };
  const written = (recipe) => ({
    version: recipe.version,
    columns: recipe.columns.map((c) => {
      const out = { name: c.name, method: c.method };
      if (KEYED.includes(c.method)) out.salt_env = c.salt_env;
      for (const key of SETTINGS[c.method] ?? []) out[key] = c[key] ?? DEFAULTS[key];
      return out;
    }),
  });
  const toYaml = (recipe) => JSON.stringify(written(recipe), null, 2);
  const wasm = {
    recipe_yaml_to_json: (text) => {
      const recipe = JSON.parse(text);
      if (recipe.version !== 1) {
        throw `recipe version ${recipe.version} is not supported: this build reads version 1`;
      }
      return JSON.stringify(written(recipe));
    },
    recipe_json_to_yaml: (json) => toYaml(JSON.parse(json)),
  };
  return { wasm, toYaml, downloads: /** @type {string[]} */ ([]) };
});

const HEADERS = ["id", "email", "age", "card", "notes"];

vi.mock("./hooks/useWasm", () => ({ useWasm: () => ({ wasm: engine.wasm, methods: [] }) }));
// The table and the side panel draw on a canvas and run Web Workers, neither of which jsdom has;
// what this file tests is the recipe state behind them and the navbar.
vi.mock("./components/TableTab", () => ({ default: () => null }));
vi.mock("./components/AnalysisOverlay", () => ({ default: () => null }));
vi.mock("./hooks/useDatasetAnalysis", () => ({
  useDatasetAnalysis: () => ({
    columnRisk: { riskByName: {}, riskLoading: false },
    diversityMatrix: { raw: null, loading: false, error: null, quasiColumns: [], allColumns: [] },
    rowFilter: { filteredRowIndices: null, setRowFilter: () => {} },
    columnSubsets: {},
    association: null,
  }),
}));
vi.mock("./state/profileClient", () => ({
  profileFile: async () => ({
    sample: { headers: HEADERS, rowCount: 3 },
    profile: { columns: HEADERS.map((name) => ({ name, inferred_type: "string" })) },
  }),
}));
vi.mock("./utils", async (importOriginal) => ({
  .../** @type {object} */ (await importOriginal()),
  downloadBlob: (content) => engine.downloads.push(content),
}));

const { default: App } = await import("./App");

beforeEach(() => {
  engine.downloads.length = 0;
});
afterEach(cleanup);

const RECIPE = {
  version: 1,
  columns: [
    { name: "email", method: "token_hmac", salt_env: "CUSTOMER_SALT" },
    { name: "age", method: "bucket", bucket_width: 5 },
    { name: "card", method: "partial_mask", keep_prefix: 2, keep_suffix: 3 },
    { name: "notes", method: "drop" },
  ],
};
const recipeFile = (text, name = "recipe.triplex.yaml") => new File([text], name);

async function renderWithTable() {
  const view = render(<App extensions={[]} initialFile={new File(["id\n1\n"], "people.csv")} />);
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Open recipe" }).hasAttribute("disabled")).toBe(
      false,
    ),
  );
  return view;
}

function chooseRecipe(container, file) {
  const input = container.querySelector('input[type="file"][accept=".yaml,.yml"]');
  fireEvent.change(input, { target: { files: [file] } });
}

const downloadRecipe = () =>
  fireEvent.click(screen.getByRole("button", { name: "Download recipe" }));

describe("Open recipe", () => {
  it("waits for a loaded table", () => {
    render(<App extensions={[]} />);
    expect(screen.getByRole("button", { name: "Open recipe" }).hasAttribute("disabled")).toBe(true);
  });

  it("gives back the same recipe when it is opened and downloaded again", async () => {
    const { container } = await renderWithTable();
    const yaml = engine.toYaml(RECIPE);
    chooseRecipe(container, recipeFile(yaml));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Download recipe" }).hasAttribute("disabled")).toBe(
        false,
      ),
    );
    downloadRecipe();
    expect(engine.downloads).toEqual([yaml]);

    // Opened again over the recipe it set, it asks first, then gives the same file once more.
    chooseRecipe(container, recipeFile(engine.downloads[0]));
    fireEvent.click(await screen.findByRole("button", { name: "Replace recipe" }));
    downloadRecipe();
    expect(engine.downloads).toEqual([yaml, yaml]);
  });

  it("hides the dropped columns, as picking drop by hand does", async () => {
    const { container } = await renderWithTable();
    chooseRecipe(container, recipeFile(engine.toYaml(RECIPE)));
    await screen.findByText(/3 rows \/ 4 columns/);
  });

  it("asks before applying a recipe whose columns are not all in the table", async () => {
    const { container } = await renderWithTable();
    const recipe = {
      version: 1,
      columns: [...RECIPE.columns, { name: "postcode", method: "drop" }],
    };
    chooseRecipe(container, recipeFile(engine.toYaml(recipe)));
    expect(
      await screen.findByText(
        /1 of 5 columns in recipe.triplex.yaml is not in this table: "postcode". Apply the other 4 columns\?/,
      ),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("button", { name: "Download recipe" }).hasAttribute("disabled")).toBe(
      true,
    );

    chooseRecipe(container, recipeFile(engine.toYaml(recipe)));
    fireEvent.click(await screen.findByRole("button", { name: "Apply 4 columns" }));
    expect(
      await screen.findByText(
        /Not applied from recipe.triplex.yaml, not in this table: "postcode"/,
      ),
    ).toBeTruthy();
  });

  it("changes nothing when no column of the recipe is in the table", async () => {
    const { container } = await renderWithTable();
    const recipe = { version: 1, columns: [{ name: "postcode", method: "drop" }] };
    chooseRecipe(container, recipeFile(engine.toYaml(recipe)));
    expect(await screen.findByText(/None of the columns in recipe.triplex.yaml/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Download recipe" }).hasAttribute("disabled")).toBe(
      true,
    );
  });

  it("says what is wrong with a recipe the engine rejects", async () => {
    const { container } = await renderWithTable();
    chooseRecipe(container, recipeFile(engine.toYaml({ ...RECIPE, version: 2 })));
    expect(
      await screen.findByText(
        "recipe.triplex.yaml could not be opened. recipe version 2 is not supported: this build reads version 1",
      ),
    ).toBeTruthy();
  });

  it("says the keyed columns still need the salt", async () => {
    const { container } = await renderWithTable();
    chooseRecipe(container, recipeFile(engine.toYaml(RECIPE)));
    expect(
      await screen.findByText(/"email" uses a keyed method. Type the salt in the column menu/),
    ).toBeTruthy();
  });

  it("opens a recipe dropped on the page", async () => {
    const { container } = await renderWithTable();
    const shell = /** @type {HTMLElement} */ (container.querySelector(".app-shell"));
    fireEvent.drop(shell, {
      dataTransfer: { types: ["Files"], files: [recipeFile(engine.toYaml(RECIPE))] },
    });
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Download recipe" }).hasAttribute("disabled")).toBe(
        false,
      ),
    );
  });

  it("refuses a dropped data file when the host supplies the datasets", async () => {
    const picker = <span>Sample dataset</span>;
    const { container } = render(
      <App
        extensions={[]}
        initialFile={new File(["id\n1\n"], "people.csv")}
        datasetPicker={picker}
      />,
    );
    await screen.findByText(/3 rows/);
    fireEvent.drop(/** @type {HTMLElement} */ (container.querySelector(".app-shell")), {
      dataTransfer: { types: ["Files"], files: [new File(["a\n"], "other.csv")] },
    });
    expect(
      await screen.findByText(/Only a recipe file \(.yaml\) can be dropped here/),
    ).toBeTruthy();
  });
});
