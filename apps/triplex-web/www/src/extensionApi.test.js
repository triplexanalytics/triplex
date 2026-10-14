import { describe, it, expect } from "vitest";
import { checkExtensions, tierDisabledReason, EXTENSION_API_VERSION } from "./extensionApi";
import { EXTENSIONS } from "./extensions";

// Partial on purpose: only the members these functions read.
/** @returns {any} */
const ext = (over = {}) => ({ apiVersion: EXTENSION_API_VERSION, id: "made", tier: 2, ...over });
/** @returns {any} */
const host = (over = {}) => ({ sample: { headers: ["a"], rowCount: 1 }, ...over });

describe("checkExtensions", () => {
  it("accepts this build's list", () => {
    expect(() => checkExtensions(EXTENSIONS)).not.toThrow();
  });

  it("accepts an empty list", () => {
    expect(checkExtensions([])).toEqual([]);
  });

  it("rejects an extension built for another API version", () => {
    expect(() => checkExtensions([ext({ apiVersion: EXTENSION_API_VERSION + 1 })])).toThrow(
      /extension API/,
    );
  });

  it("rejects a duplicate id and the reserved id of the original", () => {
    expect(() => checkExtensions([ext(), ext({ tier: 3 })])).toThrow(/used twice/);
    expect(() => checkExtensions([ext({ id: "original" })])).toThrow(/reserved/);
  });

  it("rejects two extensions for one tier", () => {
    expect(() => checkExtensions([ext(), ext({ id: "other" })])).toThrow(/tier 2/);
  });
});

describe("tierDisabledReason", () => {
  it("disables a tier this build has no extension for", () => {
    expect(tierDisabledReason(undefined, host())).toBe("Not available in this build");
  });

  it("disables a tier until a file is loaded", () => {
    expect(tierDisabledReason(ext(), host({ sample: null }))).toMatch(/Load a file/);
  });

  it("enables a tier whose extension reports no status, or ready", () => {
    expect(tierDisabledReason(ext(), host())).toBeNull();
    expect(tierDisabledReason(ext({ status: () => ({ state: "ready" }) }), host())).toBeNull();
  });

  it("shows the message of an extension that reports itself locked", () => {
    const locked = ext({ status: () => ({ state: "locked", message: "Not set up here" }) });
    expect(tierDisabledReason(locked, host())).toBe("Not set up here");
  });
});
