# Wraps this app's multi-step build (Rust → wasm-bindgen → Vite). Run `make`
# or `make help` for the target list. See README.md for what each step does
# and why (e.g. why wasm-bindgen-cli's version must match Cargo.lock's).

WASM_BINDGEN_VERSION := 0.2.127

.PHONY: help setup wasm dev build preview test clippy fmt clean

help:
	@echo "make setup    one-time toolchain setup (rustup target, wasm-bindgen-cli, npm install)"
	@echo "make wasm     build the .wasm and regenerate www/pkg/ glue"
	@echo "make dev      wasm + start the Vite dev server"
	@echo "make build    wasm + production Vite build (www/dist/)"
	@echo "make preview  serve the production build (run 'make build' first)"
	@echo "make test     cargo test for this crate/deps, plus vitest for www/"
	@echo "make clippy   cargo clippy for this crate/deps, plus eslint for www/"
	@echo "make fmt      check formatting for this crate and www/ (does not rewrite files)"
	@echo "make clean    remove www/pkg, www/dist, and this crate's target/ output"

# One-time: wasm32 target for rustc, the wasm-bindgen CLI pinned to match
# Cargo.lock's wasm-bindgen version, and this app's npm dependencies.
setup:
	rustup target add wasm32-unknown-unknown
	cargo install wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION) --locked
	cd www && npm install

# Compiles the Rust crate to wasm, then runs wasm-bindgen to generate the JS
# glue + .wasm pair the frontend imports from www/pkg/ (gitignored — this is
# how it gets (re)created). Run this after any change under src/. The
# trailing sed marks the glue as untyped for `npm run typecheck`: it's
# generated (--no-typescript, so no .d.ts) and its inferred types don't hold
# up under checkJs -- not something a hand-written ambient module
# declaration can shadow, since the .js file genuinely exists on disk.
wasm:
	cargo build -p triplex-web --target wasm32-unknown-unknown --release
	wasm-bindgen --target web --out-dir www/pkg --no-typescript \
		../../target/wasm32-unknown-unknown/release/triplex_web.wasm
	sed -i '1i // @ts-nocheck' www/pkg/triplex_web.js

# Rebuilds the wasm first (so you never chase a stale build), then starts
# Vite's dev server with hot reload.
dev: wasm
	cd www && npm run dev

# Rebuilds the wasm first, then Vite's production build into www/dist/.
build: wasm
	cd www && npm run build

# Serves the build from `make build` — run that first, this doesn't rebuild.
preview:
	cd www && npm run preview

# Rust tests for this crate and every crate it depends on (triplex-io,
# triplex-schemacast, triplex-datasetcard, triplex-anonymizer), plus this
# app's JS tests (vitest) -- hooks with real branching logic only
# (useHistory, useRowFilter), not blanket coverage.
test:
	cargo test -p triplex-io -p triplex-schemacast -p triplex-datasetcard -p triplex-anonymizer -p triplex-web
	cd www && npm test

# Same crate set as `test`, run through clippy instead, plus this app's own
# JS linting (eslint) and type-checking (tsc --noEmit over JSDoc-annotated
# .js/.jsx, no .ts files) -- clippy's JS-side counterpart, not separate
# targets. typecheck needs pkg/ built first (imports from it).
clippy: wasm
	cargo clippy -p triplex-io -p triplex-schemacast -p triplex-datasetcard -p triplex-anonymizer -p triplex-web --all-targets
	cd www && npm run lint && npm run typecheck

# Check-only, not `cargo fmt`: this crate's dependencies have pre-existing
# formatting the installed rustfmt disagrees with, so applying it here would
# rewrite lines this app never touched. Fix formatting by hand for the lines
# you actually changed instead. Same for the JS side (prettier --check, not
# --write).
fmt:
	cargo fmt -p triplex-web -- --check
	cd www && npm run format:check

# Deletes generated output only (wasm glue, Vite build, this crate's Cargo
# target/) — never touches www/node_modules, re-run `make setup` for that.
clean:
	rm -rf www/pkg www/dist
	cargo clean -p triplex-web
