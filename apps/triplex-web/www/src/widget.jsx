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

  // Two things in the app are placed against the viewport, and in a notebook the viewport is not
  // the cell. Both are corrected in styles.css from what is measured here.
  //
  // 1. Menus and tooltips use `position: fixed` with viewport coordinates. JupyterLab's panels
  //    use CSS containment, which makes the panel, not the viewport, the box those are measured
  //    from, so they would open shifted by the panel's offset (off-screen, for the column menu).
  //    A fixed probe at 0,0 shows where that box really starts.
  // 2. The dialogs are native `<dialog>`s opened with `showModal()`, which centers them in the
  //    viewport and sizes them from it. VS Code shows every cell output in one webview as tall
  //    as the notebook, so that center is far from the cell and `vh` is far taller than the
  //    screen. The widget's own box is where a dialog belongs.
  //
  // Re-measured on each interaction because both change when the notebook's own layout does (a
  // sidebar opening, the notebook scrolling).
  // ponytail: a dialog follows the cell only on the next interaction, not while the notebook
  // scrolls under it; add a scroll listener on the host if that matters.
  const frame = useRef(null);
  const probe = useRef(null);
  const measureHost = () => {
    const set = (name, px) => frame.current.style.setProperty(`--triplex-${name}`, `${px}px`);
    const origin = probe.current.getBoundingClientRect();
    set("fixed-x", origin.left);
    set("fixed-y", origin.top);
    const box = frame.current.getBoundingClientRect();
    const view = document.documentElement;
    set("frame-top", box.top);
    set("frame-left", box.left);
    set("frame-right", view.clientWidth - box.right);
    set("frame-bottom", view.clientHeight - box.bottom);
    set("frame-width", box.width);
    set("frame-height", box.height);
  };
  useEffect(measureHost, []);

  const toggleExposeMapping = (event) => {
    setExposeMapping(event.target.checked);
    model.set("expose_mapping", event.target.checked);
    model.save_changes();
  };

  return (
    <div
      className="triplex-widget"
      ref={frame}
      onPointerEnter={measureHost}
      onPointerDownCapture={measureHost}
      onKeyDownCapture={measureHost}
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
