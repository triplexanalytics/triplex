import { describe, it, expect, vi, afterEach } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { EXTENSION_API_VERSION } from "./extensionApi";
import { EXTENSIONS } from "./extensions";
import { PRO_INFO_URL } from "./components/TierPicker.const";

// No wasm in jsdom: the app renders its empty state, which is all these tests need.
vi.mock("./hooks/useWasm", () => ({ useWasm: () => ({ wasm: null, methods: [] }) }));

const { default: App } = await import("./App");

afterEach(cleanup);

const tierButton = (name) => screen.getByRole("button", { name: new RegExp(name) });

describe("App with generator extensions", () => {
  it("runs with no extensions: generator tiers link to Triplex Pro", () => {
    render(<App extensions={[]} />);
    for (const name of ["Synthetic Data Generation", "Pattern-persistent Data Generation"]) {
      const link = screen.getByRole("link", { name: new RegExp(name) });
      expect(link.getAttribute("href")).toBe(PRO_INFO_URL);
      expect(link.getAttribute("target")).toBe("_blank");
      expect(link.textContent).toMatch(/Part of Triplex Pro/);
    }
    expect(screen.queryByRole("button", { name: "Compare datasets" })).toBeNull();
    expect(screen.getByText("License: Free")).toBeTruthy();
  });

  it("mutes Tier 1 and the downloads until a file is loaded", () => {
    render(<App extensions={[]} />);
    const tier1 = tierButton("Basic Anonymization");
    expect(tier1.hasAttribute("disabled")).toBe(true);
    expect(tier1.title).toMatch(/Load a file/);
    for (const name of ["Download contract", "Download recipe", "Download data"]) {
      expect(screen.getByRole("button", { name }).hasAttribute("disabled")).toBe(true);
    }
    expect(screen.getByRole("button", { name: "Choose file" }).hasAttribute("disabled")).toBe(
      false,
    );
  });

  it("this build ships no generator extensions", () => {
    expect(EXTENSIONS).toEqual([]);
  });

  it("calls each extension's hook with the loaded file", () => {
    const seen = [];
    /** @type {import("./extensionApi").GeneratorExtension} */
    const fake = {
      apiVersion: EXTENSION_API_VERSION,
      id: "made",
      tier: /** @type {2} */ (2),
      label: "Made",
      noun: "made",
      emptyHint: "Make data first",
      useGenerator: (host) => {
        seen.push(host);
        return {
          result: null,
          clearResult: () => {},
          reset: () => {},
          downloadCsv: () => {},
          downloadReport: () => {},
        };
      },
      Dialog: () => null,
      history: { label: "Make data", detail: () => "" },
    };
    render(<App extensions={[fake]} />);
    expect(seen.length).toBeGreaterThan(0);
    expect(seen.at(-1)).toMatchObject({ sample: null, fileBytes: null });
    // Tier 2 has an extension but no file yet; tier 3 has none, so it links to Triplex Pro.
    expect(tierButton("Synthetic Data Generation").title).toMatch(/Load a file/);
    expect(
      screen.getByRole("link", { name: /Pattern-persistent Data Generation/ }).getAttribute("href"),
    ).toBe(PRO_INFO_URL);
    expect(screen.getByRole("button", { name: "Compare datasets" })).toBeTruthy();
    expect(screen.getByText("License: Pro")).toBeTruthy();
  });
});
