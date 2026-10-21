import { defineConfig, mergeConfig } from "vite";
import { fileURLToPath } from "node:url";
import base from "./vite.config.js";
import { CONTENT_SECURITY_POLICY, HEADERS_FILE } from "./src/demo/contentSecurityPolicy.js";

// Builds the hosted demo (demo.html, entry src/demo.jsx) into dist-demo/, ready for a static
// host: the page as index.html, and a _headers file with the content security policy.

// Build only: the dev server injects an inline script of its own, which the policy forbids.
const demoPage = {
  name: "demo-page",
  apply: "build",
  // After Vite's own HTML plugin, which adds the page to the bundle.
  enforce: "post",
  transformIndexHtml: () => [
    {
      tag: "meta",
      attrs: { "http-equiv": "Content-Security-Policy", content: CONTENT_SECURITY_POLICY },
      injectTo: "head-prepend",
    },
  ],
  generateBundle(_options, bundle) {
    const page = bundle["demo.html"];
    if (!page) return;
    page.fileName = "index.html";
    this.emitFile({ type: "asset", fileName: "_headers", source: HEADERS_FILE });
  },
};

export default mergeConfig(
  base,
  defineConfig({
    plugins: [demoPage],
    build: {
      outDir: "dist-demo",
      rollupOptions: { input: fileURLToPath(new URL("./demo.html", import.meta.url)) },
    },
  }),
);
