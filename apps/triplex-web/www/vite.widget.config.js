import { defineConfig, mergeConfig } from "vite";
import { fileURLToPath } from "node:url";
import base from "./vite.config.js";

const src = (path) => fileURLToPath(new URL(`./src/${path}`, import.meta.url));

// wasm-bindgen's glue falls back to `new URL("triplex_web_bg.wasm", import.meta.url)` when
// init() is given no URL. This app always gives one, but Vite inlines the file for that unused
// branch anyway, once per bundle: the page and each of the four workers, 5 MB apiece.
const GLUE_DEFAULT_WASM_URL = "new URL('triplex_web_bg.wasm', import.meta.url)";
const dropGlueDefaultWasmUrl = {
  name: "drop-glue-default-wasm-url",
  enforce: "pre",
  transform(code, id) {
    if (!id.endsWith("/pkg/triplex_web.js")) return null;
    if (!code.includes(GLUE_DEFAULT_WASM_URL)) {
      this.error("wasm-bindgen glue changed: its default .wasm URL was not found");
    }
    return code.replace(GLUE_DEFAULT_WASM_URL, "undefined");
  },
};

// Builds the Jupyter widget (src/widget.jsx) into dist-widget/widget.js + style.css.
// anywidget loads exactly one JS module and one stylesheet and can serve nothing else, so
// unlike the standalone build this one has no code-split chunks, and the workers and the .wasm
// are inlined into the JS file.
export default mergeConfig(
  base,
  defineConfig({
    // Library mode leaves this for the consumer's bundler to fill in; there is none here.
    define: { "process.env.NODE_ENV": JSON.stringify("production") },
    plugins: [dropGlueDefaultWasmUrl],
    worker: { plugins: () => [dropGlueDefaultWasmUrl] },
    resolve: {
      // Matched against the import specifier as written, so these two names must stay unique
      // to state/workers.js and worker/wasmUrl.js.
      alias: [
        { find: /^\.\/workers$/, replacement: src("state/workers.widget.js") },
        { find: /^\.\/wasmUrl$/, replacement: src("worker/wasmUrl.widget.js") },
      ],
    },
    build: {
      outDir: "dist-widget",
      lib: { entry: src("widget.jsx"), formats: ["es"], fileName: "widget" },
      rollupOptions: { output: { inlineDynamicImports: true } },
    },
  }),
);
