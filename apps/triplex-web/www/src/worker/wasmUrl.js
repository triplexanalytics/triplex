// Where a worker loads the .wasm from. Its own module so the Jupyter widget build
// (vite.widget.config.js) can swap it for wasmUrl.widget.js.
export { default } from "../../pkg/triplex_web_bg.wasm?url";
