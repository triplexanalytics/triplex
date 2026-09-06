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
	@echo "make test     cargo test for this crate and everything it depends on"
	@echo "make clippy   cargo clippy for this crate and everything it depends on"
	@echo "make fmt      check this crate's formatting (does not rewrite files)"
	@echo "make clean    remove www/pkg, www/dist, and this crate's target/ output"

# One-time: wasm32 target for rustc, the wasm-bindgen CLI pinned to match
# Cargo.lock's wasm-bindgen version, and this app's npm dependencies.
setup:
	rustup target add wasm32-unknown-unknown
	cargo install wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION) --locked
	cd www && npm install

# Compiles the Rust crate to wasm, then runs wasm-bindgen to generate the JS
# glue + .wasm pair the frontend imports from www/pkg/ (gitignored — this is
# how it gets (re)created). Run this after any change under src/.
wasm:
	cargo build -p triplex-web --target wasm32-unknown-unknown --release
	wasm-bindgen --target web --out-dir www/pkg --no-typescript \
		../../target/wasm32-unknown-unknown/release/triplex_web.wasm

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
# triplex-schemacast, triplex-datasetcard, triplex-anonymizer) — no JS tests exist yet.
test:
	cargo test -p triplex-io -p triplex-schemacast -p triplex-datasetcard -p triplex-anonymizer -p triplex-web

# Same crate set as `test`, run through clippy instead.
clippy:
	cargo clippy -p triplex-io -p triplex-schemacast -p triplex-datasetcard -p triplex-anonymizer -p triplex-web --all-targets

# Check-only, not `cargo fmt`: this crate's dependencies have pre-existing
# formatting the installed rustfmt disagrees with, so applying it here would
# rewrite lines this app never touched. Fix formatting by hand for the lines
# you actually changed instead.
fmt:
	cargo fmt -p triplex-web -- --check

# Deletes generated output only (wasm glue, Vite build, this crate's Cargo
# target/) — never touches www/node_modules, re-run `make setup` for that.
clean:
	rm -rf www/pkg www/dist
	cargo clean -p triplex-web
