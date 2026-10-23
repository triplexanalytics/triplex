import { describe, it, expect, vi, afterEach, beforeAll } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MUNICIPAL_RECIPE } from "../testing/fakeEngine";

// The engine is faked (testing/fakeEngine.js) and the canvas-drawn parts are stand-ins, as in
// App.compare.test.jsx; the dataset is the demo's own municipal file.
vi.mock("../hooks/useWasm", async () => {
  const { fakeWasm } = await import("../testing/fakeEngine");
  return { useWasm: () => ({ wasm: fakeWasm, methods: [] }) };
});
vi.mock("../state/profileClient", async () => {
  const { fakeProfile } = await import("../testing/fakeEngine");
  return { profileFile: async (bytes) => fakeProfile(bytes) };
});
vi.mock("../state/anonymizeClient", async () => {
  const { fakeAnonymize } = await import("../testing/fakeEngine");
  return {
    runAnonymize: async (...args) => fakeAnonymize(...args),
    checkPatternConformance: async () => "{}",
    convertBytes: async (bytes) => bytes,
  };
});
vi.mock("../state/datasetCardClient", () => ({
  scoreColumnRisk: async () => ({}),
  scoreDiversityMatrix: async () => ({}),
}));
vi.mock("../hooks/useDatasetAnalysis", () => ({
  useDatasetAnalysis: () => ({
    columnRisk: { riskByName: {}, riskLoading: false },
    diversityMatrix: { raw: null, loading: false, error: null, quasiColumns: [], allColumns: [] },
    rowFilter: { filteredRowIndices: null, setRowFilter: () => {} },
    columnSubsets: {},
    association: null,
  }),
}));
vi.mock("../components/TableTab", () => ({
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
vi.mock("../components/AnalysisOverlay", () => ({ default: () => null }));
vi.mock("../components/ColumnCard", () => ({ default: ({ col }) => <div>card {col.name}</div> }));
vi.mock("../components/DiversityCheckSection", () => ({ DiversityCompare: () => null }));

const { default: DemoApp } = await import("./DemoApp");

afterEach(() => {
  cleanup();
  window.location.hash = "";
});
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
    this.dispatchEvent(new Event("close"));
  };
});

describe("DemoApp: Compare", () => {
  it("reaches the anonymized comparison of the municipal dataset after a run", async () => {
    const { container } = render(<DemoApp extensions={[]} />);
    const openRecipe = screen.getByRole("button", { name: "Open recipe" });
    await waitFor(() => expect(openRecipe.hasAttribute("disabled")).toBe(false), {
      timeout: 5000,
    });
    const compare = screen.getByRole("button", { name: "Compare datasets" });
    expect(compare.hasAttribute("disabled")).toBe(true);

    const input = container.querySelector('input[type="file"][accept=".yaml,.yml"]');
    fireEvent.change(input, {
      target: { files: [new File([JSON.stringify(MUNICIPAL_RECIPE)], "recipe.triplex.yaml")] },
    });
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Download recipe" }).hasAttribute("disabled")).toBe(
        false,
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Run anonymize" }));
    fireEvent.click(await screen.findByRole("button", { name: "Compare with the original" }));

    const dialog = await screen.findByRole("dialog", { name: "Compare datasets" });
    const gaps = [...dialog.querySelectorAll(".compare-gap")].map((el) => el.textContent);
    expect(gaps).toEqual(
      ["name", "email", "start_date"].map(
        (c) => `${c} is not in the anonymized dataset: the recipe drops it.`,
      ),
    );

    // On postcode, birth year and sex: the counts `triplex risk` gives for the same two files.
    fireEvent.click(within(dialog).getByRole("tab", { name: "Privacy matrices" }));
    const chips = within(dialog).getByRole("group", { name: "Quasi-identifiers" });
    for (const name of ["municipality", "household", "scheme"]) {
      fireEvent.click(within(chips).getByRole("button", { name }));
    }
    await waitFor(() =>
      expect(
        [...dialog.querySelectorAll(".compare-alone strong")].map((el) => el.textContent),
      ).toEqual(["723 of 1,200", "9 of 1,200"]),
    );
  });
});
