# triplex-web

Browser UI over SchemaCast/Anonymizer. Everything runs
client-side via WebAssembly: no server, no upload, no telemetry. The UI
itself (`www/`) is a Vite + React app — the repo's first JS toolchain,
adopted because its table view uses Glide Data Grid (React-only) and its
components must also mount inside a Python anywidget later (which expects a
bundled JS asset anyway).

## Commands

`Makefile` wraps every command below — `make help` lists them. One-time
setup, then day-to-day work is just `make dev` / `make build` / `make test`.
The repo root also has a `Makefile` that fans the same targets out across
every app that has one (just this one today) — `make test` from the repo
root runs this app's tests too.

## Build

One-time setup — `make setup`, or by hand:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked
cd www && npm install
```

`wasm-bindgen-cli`'s version must exactly match the `wasm-bindgen` crate
version pinned in `Cargo.toml` (and in the `Makefile`) — if that version
changes, reinstall the CLI to match.

Build the `.wasm` and generate its JS glue into `www/pkg/` (gitignored,
regenerate after every Rust source change) — `make wasm`, or by hand:

```sh
cargo build -p triplex-web --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir www/pkg --no-typescript \
  ../../target/wasm32-unknown-unknown/release/triplex_web.wasm
```

## Run

`make dev` (rebuilds the wasm first, so you never chase a stale build), or
by hand:

```sh
cd www && npm run dev
```

For a production build: `make build`, then `make preview` — or
`npm run build && npm run preview` from `www/`. Worker bundling and the wasm
asset URL are genuinely different code paths between dev and build, so check
both after any change to `src/worker/` or `src/wasm.js`.

For the Jupyter widget: `make widget` builds the same app as one JS file
plus one stylesheet into `www/dist-widget/` (`www/vite.widget.config.js`,
entry `www/src/widget.jsx`), with the workers and the `.wasm` inlined,
because anywidget can load nothing else. `packages/triplex-jupyter` ships
that bundle. Two rules keep it working: every rule in `www/src/styles.css`
stays inside `.app-shell` (the sheet is loaded into a notebook page this app
does not own; page-level rules go in `page.css`), and workers are created
only in `www/src/state/workers.js`.

`make test`/`make clippy`/`make fmt` run against this crate and everything
it depends on (`triplex-io`, `triplex-schemacast`, `triplex-anonymizer`).
`make clean` removes `www/pkg`, `www/dist`, `www/dist-widget`, and this crate's `target/`
output.

## Scope

`.csv`/`.tsv`/`.jsonl` only — `.parquet` input isn't supported here. Its
codecs (zstd/snap) need a C toolchain that `wasm32-unknown-unknown` doesn't
have; see `crates/triplex-io/Cargo.toml` for the per-target dependency
split that keeps native builds (the CLI) unaffected.

Tier 1 ("Basic Anonymization") — one algorithm per column, picked from
whatever `triplex-anonymizer::AnonymizeMethod::ALL` currently implements
(just `token_hmac` today). Tier 2 ("Synthetic Data Generation") opens a
dialog (standard Gaussian copula, or differentially private with a
user-declared epsilon and public bounds) and the result becomes a Synthetic
tab beside Original. Tier 3 is shown in the tier
picker but disabled.
