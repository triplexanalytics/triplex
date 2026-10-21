import { describe, it, expect, vi, afterEach } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { CONTENT_SECURITY_POLICY, HEADERS_FILE } from "./contentSecurityPolicy";
import { siteLink } from "./links";

// No wasm in jsdom; these tests are about the demo's chrome, not the engine.
vi.mock("../hooks/useWasm", () => ({ useWasm: () => ({ wasm: null, methods: [] }) }));

const { default: DemoApp } = await import("./DemoApp");

afterEach(() => {
  cleanup();
  window.location.hash = "";
});

describe("DemoApp", () => {
  it("offers the sample datasets and no way to load a file", () => {
    const { container } = render(<DemoApp extensions={[]} />);
    expect(container.querySelector('input[type="file"]')).toBeNull();
    expect(screen.queryByRole("button", { name: "Choose file" })).toBeNull();
    expect(container.querySelector(".dropzone")).toBeNull();
    const picker = screen.getByRole("combobox", { name: /Sample dataset/ });
    expect([...picker.querySelectorAll("option")].map((o) => o.value)).toEqual([
      "municipal",
      "statistical",
    ]);
  });

  it("opens the dataset the address names, and the picker changes the address", async () => {
    window.location.hash = "#dataset=statistical";
    render(<DemoApp extensions={[]} />);
    const picker = /** @type {HTMLSelectElement} */ (
      screen.getByRole("combobox", { name: /Sample dataset/ })
    );
    expect(picker.value).toBe("statistical");
    fireEvent.change(picker, { target: { value: "municipal" } });
    await waitFor(() => expect(window.location.hash).toBe("#dataset=municipal"));
  });

  it("says plainly that it is a demo and why own files are off", () => {
    render(<DemoApp extensions={[]} />);
    expect(
      screen.getByRole("heading", {
        name: "This is a demo. Loading your own files is switched off.",
      }),
    ).toBeTruthy();
    expect(screen.getByText(/stay on your own premises/)).toBeTruthy();
    expect(screen.getByText("Own files: off in the demo")).toBeTruthy();
  });

  it("says Tiers 2 and 3 are Triplex Pro only when the build has them", () => {
    render(<DemoApp extensions={[]} />);
    expect(screen.queryByText(/paid extension, shown here/)).toBeNull();
    expect(screen.queryByText(/provided here for evaluation only/)).toBeNull();
  });

  it("tags its links back to the site with where they came from", () => {
    render(<DemoApp extensions={[]} />);
    const install = screen.getByRole("link", { name: "install Triplex" });
    const url = new URL(install.getAttribute("href"));
    expect(url.searchParams.get("utm_source")).toBe("demo");
    expect(url.searchParams.get("utm_content")).toBe("install");
    expect(siteLink("https://triplexanalytics.io/docs/web.html", "pro")).toMatch(/utm_content=pro/);
  });
});

describe("content security policy", () => {
  it("names no origin other than the demo's own", () => {
    const sources = CONTENT_SECURITY_POLICY.split(";").flatMap((d) =>
      d.trim().split(/\s+/).slice(1),
    );
    const allowed = ["'self'", "'none'", "'unsafe-inline'", "'wasm-unsafe-eval'", "data:", "blob:"];
    expect(sources.filter((s) => !allowed.includes(s))).toEqual([]);
    expect(CONTENT_SECURITY_POLICY).toMatch(/connect-src 'self'(;|$)/);
    expect(CONTENT_SECURITY_POLICY).toMatch(/default-src 'self'(;|$)/);
  });

  it("is sent as a header too, with framing forbidden and indexing off", () => {
    expect(HEADERS_FILE).toContain(`Content-Security-Policy: ${CONTENT_SECURITY_POLICY}`);
    expect(HEADERS_FILE).toContain("frame-ancestors 'none'");
    expect(HEADERS_FILE).toContain("X-Robots-Tag: noindex");
  });

  it("the demo page itself asks not to be indexed", () => {
    const page = readFileSync("demo.html", "utf8");
    expect(page).toContain('<meta name="robots" content="noindex" />');
  });
});
