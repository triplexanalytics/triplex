import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { execSync } from "node:child_process";
import process from "node:process";

// The UI's version is its latest `web-v*` git tag, not package.json's
// placeholder. Exposed to the app as import.meta.env.VITE_APP_VERSION;
// empty when git or the tags are missing, and the navbar then shows none.
try {
  process.env.VITE_APP_VERSION = execSync("git describe --tags --match 'web-v*'", {
    stdio: ["ignore", "pipe", "ignore"],
  })
    .toString()
    .trim()
    .replace(/^web-v/, "");
} catch {
  process.env.VITE_APP_VERSION = "";
}

export default defineConfig({
  plugins: [react()],
  // wasm-bindgen's `--target web` glue is an ES module — classic workers
  // can't `import` it, so the bundled worker output must stay ESM too.
  worker: { format: "es" },
  test: {
    environment: "jsdom",
  },
});
