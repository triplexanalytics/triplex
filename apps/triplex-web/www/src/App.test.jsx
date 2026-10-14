import { describe, it, expect, vi, afterEach } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { EXTENSION_API_VERSION } from "./extensionApi";

// No wasm in jsdom: the app renders its empty state, which is all these tests need.
vi.mock("./hooks/useWasm", () => ({ useWasm: () => ({ wasm: null, methods: [] }) }));

const { default: App } = await import("./App");

afterEach(cleanup);

const tierButton = (name) => screen.getByRole("button", { name: new RegExp(name) });

describe("App with generator extensions", () => {
  it("runs with no extensions: generator tiers are disabled, not missing", () => {
    render(<App extensions={[]} />);
    for (const name of ["Synthetic Data Generation", "Pattern-persistent Data Generation"]) {
      expect(tierButton(name)).toHaveProperty("disabled", true);
      expect(tierButton(name)).toHaveProperty("title", "Not available in this build");
    }
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
    // Tier 2 has an extension but no file yet; tier 3 has none in this build.
    expect(tierButton("Synthetic Data Generation").title).toMatch(/Load a file/);
    expect(tierButton("Pattern-persistent Data Generation").title).toBe(
      "Not available in this build",
    );
  });
});
