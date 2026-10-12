import { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

/** The app plus the one control that only makes sense inside a notebook: whether the mapping
 * file may go to the kernel. `model` is the widget's state, synced with the Python kernel.
 *
 * Read from it: `data` (the dataset's bytes), `file_name` (its extension tells the engine the
 * format) and `expose_mapping`. Written to it: the keys `App` reports through `onHostState`,
 * under the same names, and `expose_mapping` when the user changes it here. */
// eslint-disable-next-line react-refresh/only-export-components -- never hot-reloaded: this file is the notebook bundle's entry
function Host({ model }) {
  const [data, setData] = useState(() => model.get("data"));
  const [exposeMapping, setExposeMapping] = useState(() => !!model.get("expose_mapping"));
  const [appState, setAppState] = useState(/** @type {Record<string, any>} */ ({}));

  useEffect(() => {
    const onData = () => setData(model.get("data"));
    const onExpose = () => setExposeMapping(!!model.get("expose_mapping"));
    model.on("change:data", onData);
    model.on("change:expose_mapping", onExpose);
    return () => {
      model.off("change:data", onData);
      model.off("change:expose_mapping", onExpose);
    };
  }, [model]);

  const file = useMemo(
    () => (data?.byteLength ? new File([data], model.get("file_name")) : null),
    [data, model],
  );

  // Send only what changed: every key goes through the kernel connection, and some are whole
  // tables. The mapping file reverses the pseudonyms, so it is held back unless the user opted
  // in, and withdrawn again when they opt out.
  const sent = useRef({ mapping_csv: null });
  useEffect(() => {
    const state = {
      ...appState,
      mapping_csv: exposeMapping ? (appState.mapping_csv ?? null) : null,
    };
    const changed = Object.keys(state).filter((key) => state[key] !== sent.current[key]);
    sent.current = state;
    if (changed.length === 0) return;
    for (const key of changed) model.set(key, state[key]);
    model.save_changes();
  }, [appState, exposeMapping, model]);

  // JupyterLab's panels use CSS containment, which makes the panel, not the viewport, the box
  // that `position: fixed` is measured from. The app places its menus and tooltips with viewport
  // coordinates, so in a notebook they would open shifted by the panel's offset (off-screen, for
  // the column menu). A fixed probe at 0,0 shows where that box really starts, and styles.css
  // shifts every fixed element back by that much. Re-measured on each interaction because the
  // offset changes when the notebook's own layout does (a sidebar opening, say).
  const frame = useRef(null);
  const probe = useRef(null);
  const measureFixedOrigin = () => {
    const origin = probe.current.getBoundingClientRect();
    frame.current.style.setProperty("--triplex-fixed-x", `${origin.left}px`);
    frame.current.style.setProperty("--triplex-fixed-y", `${origin.top}px`);
  };
  useEffect(measureFixedOrigin, []);

  const toggleExposeMapping = (event) => {
    setExposeMapping(event.target.checked);
    model.set("expose_mapping", event.target.checked);
    model.save_changes();
  };

  return (
    <div
      className="triplex-widget"
      ref={frame}
      onPointerEnter={measureFixedOrigin}
      onPointerDownCapture={measureFixedOrigin}
    >
      <div className="triplex-widget-probe" ref={probe} />
      <label className="triplex-widget-option">
        <input type="checkbox" checked={exposeMapping} onChange={toggleExposeMapping} />
        Send the mapping file to the notebook kernel. It reverses the pseudonyms; leave this off
        unless your code needs it.
      </label>
      <div className="triplex-widget-app">
        <App initialFile={file} onHostState={setAppState} />
      </div>
    </div>
  );
}

/** Jupyter entry point: the module anywidget loads (https://anywidget.dev/en/afm/). Built by
 * `vite.widget.config.js` into one JS file and one CSS file, which the Python package
 * `triplex-jupyter` ships. */
function render({ model, el }) {
  el.style.height = model.get("height");
  const root = createRoot(el);
  root.render(<Host model={model} />);
  return () => root.unmount();
}

export default { render };
