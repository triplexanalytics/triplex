import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import Navbar from "./Navbar";

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

// Only what these tests need; the rest of the props fall back to "not available".
const props = /** @type {any} */ ({
  onCompare: () => {},
  canCompare: true,
  tierDisabledReasons: {},
  proTiers: [],
  activeTier: 1,
  rowCount: null,
});
const compare = () => screen.getByRole("button", { name: "Compare datasets" });
const dot = () => compare().querySelector(".pulse-dot");

describe("Navbar", () => {
  it("puts every operation in one toolbar, each with an icon", () => {
    render(<Navbar {...props} canDownloadData={false} />);
    const bar = screen.getByRole("toolbar", { name: "Operations" });
    const buttons = [...bar.querySelectorAll("button")];
    expect(buttons.map((b) => b.textContent)).toEqual([
      "Open recipe",
      "Download recipe",
      "Download contract",
      "Compare datasets",
      "Download data",
    ]);
    for (const b of buttons) expect(b.querySelector("svg[aria-hidden='true']")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Choose file" }).querySelector("svg")).toBeTruthy();
  });

  it("pulses a dot next to Compare for ten seconds after a new result", () => {
    const { rerender } = render(<Navbar {...props} comparePulse={null} />);
    expect(dot()).toBeNull();

    rerender(<Navbar {...props} comparePulse={{ run: 1 }} />);
    expect(dot()).toBeTruthy();
    act(() => vi.advanceTimersByTime(9_900));
    expect(dot()).toBeTruthy();
    act(() => vi.advanceTimersByTime(100));
    expect(dot()).toBeNull();

    // A newer result pulses again; opening Compare ends it early.
    rerender(<Navbar {...props} comparePulse={{ run: 2 }} />);
    expect(dot()).toBeTruthy();
    fireEvent.click(compare());
    expect(dot()).toBeNull();
  });

  it("stops when the result goes away", () => {
    const { rerender } = render(<Navbar {...props} comparePulse={{ run: 1 }} />);
    rerender(<Navbar {...props} comparePulse={null} />);
    expect(dot()).toBeNull();
  });
});
