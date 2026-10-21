import { createRoot } from "react-dom/client";
import DemoApp from "./demo/DemoApp";
import { EXTENSIONS } from "./extensions";
import "./styles.css";
import "./page.css";
import "./demo/demo.css";

// The hosted demo's entry point (demo.html, built by vite.demo.config.js). It imports the
// extensions here, at the same path as App.jsx, so a build that swaps "./extensions" swaps them
// for the demo too.
createRoot(document.getElementById("root")).render(<DemoApp extensions={EXTENSIONS} />);
