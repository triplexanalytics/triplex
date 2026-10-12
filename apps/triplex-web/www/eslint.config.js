import js from "@eslint/js";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import globals from "globals";
import prettier from "eslint-config-prettier";

export default [
  { ignores: ["dist", "dist-widget", "pkg"] },
  js.configs.recommended,
  react.configs.flat.recommended,
  react.configs.flat["jsx-runtime"],
  {
    files: ["**/*.{js,jsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: { ...globals.browser, ...globals.worker },
    },
    settings: { react: { version: "18.3" } },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      // Only the two classic hooks rules (real bugs: hook-order violations,
      // stale closures from missing deps) -- not react-hooks' full v7
      // "recommended" set, which also bundles React Compiler-readiness
      // lints (e.g. set-state-in-effect) this codebase's synchronous
      // reset-on-input-change effects trip on and that don't apply here,
      // since this app isn't opted into the compiler.
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
      "no-unused-vars": ["warn", { argsIgnorePattern: "^_" }],
      // No PropTypes convention anywhere in this codebase -- JSDoc +
      // `@ts-check` is the chosen type-safety layer instead, not this.
      "react/prop-types": "off",
      // Pedantic: forces HTML entities for apostrophes/quotes in JSX text,
      // which reads worse than the plain character it replaces.
      "react/no-unescaped-entities": "off",
    },
  },
  prettier,
];
