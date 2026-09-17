import { describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useHistory } from "./useHistory";

describe("useHistory", () => {
  it("logs entries newest first", () => {
    const { result } = renderHook(() => useHistory());
    act(() => result.current.log("Load dataset", "3 rows", null, () => {}));
    act(() => result.current.log("Declare as string", "1 col", "email", () => {}));

    expect(result.current.entries.map((e) => e.op)).toEqual(["Declare as string", "Load dataset"]);
    expect(result.current.entries[0].column).toBe("email");
    expect(result.current.entries[1].column).toBeNull();
  });

  it("undo runs the entry's captured closure and removes only that entry", () => {
    const { result } = renderHook(() => useHistory());
    const undoFirst = vi.fn();
    const undoSecond = vi.fn();
    act(() => result.current.log("op1", "1", null, undoFirst));
    act(() => result.current.log("op2", "1", null, undoSecond));
    const secondId = result.current.entries[0].id;

    act(() => result.current.undo(secondId));

    expect(undoSecond).toHaveBeenCalledTimes(1);
    expect(undoFirst).not.toHaveBeenCalled();
    expect(result.current.entries).toHaveLength(1);
    expect(result.current.entries[0].op).toBe("op1");
  });

  it("undo on an id that isn't present is a no-op", () => {
    const { result } = renderHook(() => useHistory());
    act(() => result.current.log("op1", "1", null, () => {}));

    expect(() => act(() => result.current.undo(999))).not.toThrow();
    expect(result.current.entries).toHaveLength(1);
  });

  it("clear empties the log", () => {
    const { result } = renderHook(() => useHistory());
    act(() => result.current.log("op1", "1", null, () => {}));
    act(() => result.current.clear());

    expect(result.current.entries).toEqual([]);
  });

  it("ids keep incrementing across entries even after some are removed", () => {
    const { result } = renderHook(() => useHistory());
    act(() => result.current.log("op1", "1", null, () => {}));
    const firstId = result.current.entries[0].id;
    act(() => result.current.undo(firstId));
    act(() => result.current.log("op2", "1", null, () => {}));

    expect(result.current.entries[0].id).not.toBe(firstId);
  });
});
