import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

/** Mounts the whole app into a container element the host provides — the
 * standalone page below calls this with `#root`, and a future Python
 * anywidget host calls it with whatever node it owns. No assumption that
 * `document.body`/a fixed `#root` id exists, per the cross-surface reuse
 * constraint in the design notes. */
export function mount(container) {
  const root = createRoot(container);
  root.render(<App />);
  return () => root.unmount();
}

const standaloneRoot = document.getElementById("root");
if (standaloneRoot) {
  mount(standaloneRoot);
}
