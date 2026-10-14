import { describe, it, expect, vi, afterEach } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import DatasetTabs from "./DatasetTabs";

afterEach(cleanup);

describe("DatasetTabs", () => {
  it("shows one tab per dataset, disabled with its reason until it exists", () => {
    const onSelect = vi.fn();
    render(
      <DatasetTabs
        active="original"
        onSelect={onSelect}
        datasets={[
          { id: "original", label: "Original", disabledReason: null },
          { id: "made", label: "Made", disabledReason: "Make data first" },
        ]}
      />,
    );
    expect(screen.getAllByRole("tab")).toHaveLength(2);
    const made = screen.getByRole("tab", { name: "Made" });
    expect(made).toHaveProperty("disabled", true);
    expect(made.title).toBe("Make data first");
    fireEvent.click(screen.getByRole("tab", { name: "Original" }));
    expect(onSelect).toHaveBeenCalledWith("original");
  });

  it("shows only the original when the build has no generators", () => {
    render(
      <DatasetTabs
        active="original"
        onSelect={() => {}}
        datasets={[{ id: "original", label: "Original", disabledReason: null }]}
      />,
    );
    expect(screen.getAllByRole("tab")).toHaveLength(1);
  });
});
