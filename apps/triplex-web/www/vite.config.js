import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  // wasm-bindgen's `--target web` glue is an ES module — classic workers
  // can't `import` it, so the bundled worker output must stay ESM too.
  worker: { format: "es" },
});
