import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, fireEvent } from "@testing-library/react";

// The real App needs wasm and Web Workers; this test is about the widget's contract with its
// model, so App is replaced by a probe that records its props.
let appProps;
vi.mock("./App", () => ({
  default: (props) => {
    appProps = props;
    return null;
  },
}));

const { default: widget } = await import("./widget");

function fakeModel(initial) {
  const state = { ...initial };
  const listeners = {};
  return {
    get: (key) => state[key],
    set: vi.fn((key, value) => {
      state[key] = value;
    }),
    save_changes: vi.fn(),
    on: (event, fn) => {
      listeners[event] = fn;
    },
    off: (event) => {
      delete listeners[event];
    },
    emit: (event) => listeners[event]?.(),
    state,
  };
}

const bytes = (text) => new DataView(new TextEncoder().encode(text).buffer);
const MAPPING = new Uint8Array([7]);

function show(initial) {
  const model = fakeModel({
    data: bytes("a,b\n1,2\n"),
    file_name: "data.csv",
    height: "500px",
    expose_mapping: false,
    ...initial,
  });
  const el = document.createElement("div");
  let cleanup;
  act(() => {
    cleanup = widget.render({ model, el });
  });
  return {
    model,
    el,
    cleanup,
    checkbox: /** @type {HTMLInputElement} */ (el.querySelector("input[type=checkbox]")),
  };
}

const report = (state) => act(() => appProps.onHostState(state));

describe("widget.render", () => {
  beforeEach(() => {
    appProps = null;
  });

  it("hands the model's dataset to App as a named File", async () => {
    const { el } = show();
    expect(el.style.height).toBe("500px");
    expect(appProps.initialFile.name).toBe("data.csv");
    expect(await appProps.initialFile.text()).toBe("a,b\n1,2\n");
  });

  it("passes no file when the model has no data", () => {
    show({ data: null });
    expect(appProps.initialFile).toBeNull();
  });

  it("reloads when the kernel replaces the dataset", async () => {
    const { model } = show();
    model.state.data = bytes("x\n9\n");
    act(() => model.emit("change:data"));
    expect(await appProps.initialFile.text()).toBe("x\n9\n");
  });

  it("syncs only the keys that changed", () => {
    const { model } = show();
    const csv = new Uint8Array([1]);
    report({ recipe_yaml: "r1", masked_csv: csv });
    expect(model.set.mock.calls).toEqual([
      ["recipe_yaml", "r1"],
      ["masked_csv", csv],
    ]);
    expect(model.save_changes).toHaveBeenCalledTimes(1);

    report({ recipe_yaml: "r2", masked_csv: csv });
    expect(model.set).toHaveBeenLastCalledWith("recipe_yaml", "r2");
    expect(model.set).toHaveBeenCalledTimes(3);

    report({ recipe_yaml: "r2", masked_csv: csv });
    expect(model.save_changes).toHaveBeenCalledTimes(2);
  });

  it("keeps the mapping file in the page unless the user opted in", () => {
    const { model, checkbox } = show();
    expect(checkbox.checked).toBe(false);
    report({ recipe_yaml: "r", mapping_csv: MAPPING });
    expect(model.set.mock.calls).toEqual([["recipe_yaml", "r"]]);
  });

  it("sends the mapping file when the kernel asked for it", () => {
    const { model, checkbox } = show({ expose_mapping: true });
    expect(checkbox.checked).toBe(true);
    report({ mapping_csv: MAPPING });
    expect(model.set).toHaveBeenCalledWith("mapping_csv", MAPPING);
  });

  it("sends the mapping file once the user ticks the option, and withdraws it when unticked", () => {
    const { model, checkbox } = show();
    report({ mapping_csv: MAPPING });
    expect(model.set).not.toHaveBeenCalled();

    fireEvent.click(checkbox);
    expect(model.set.mock.calls).toEqual([
      ["expose_mapping", true],
      ["mapping_csv", MAPPING],
    ]);

    fireEvent.click(checkbox);
    expect(model.set).toHaveBeenLastCalledWith("mapping_csv", null);
    expect(model.state.expose_mapping).toBe(false);
  });

  it("follows the kernel changing the option", () => {
    const { model, checkbox } = show();
    report({ mapping_csv: MAPPING });
    model.state.expose_mapping = true;
    act(() => model.emit("change:expose_mapping"));
    expect(checkbox.checked).toBe(true);
    expect(model.set).toHaveBeenCalledWith("mapping_csv", MAPPING);
  });

  it("stops listening and unmounts on cleanup", () => {
    const { model, el, cleanup } = show();
    act(() => cleanup());
    expect(model.emit("change:data")).toBeUndefined();
    expect(el.innerHTML).toBe("");
  });
});
