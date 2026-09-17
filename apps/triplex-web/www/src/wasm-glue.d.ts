// www/pkg/ is wasm-bindgen-generated glue (gitignored, rebuilt by `make
// wasm`) -- not ours to type-check, and its own inferred types don't hold up
// under checkJs. No body = TS treats every import from it as `any`.
declare module "*/pkg/triplex_web.js";
