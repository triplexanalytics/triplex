# triplex-web

Browser UI over SchemaCast/Anonymizer. Everything runs
client-side via WebAssembly: no server, no upload, no telemetry. The UI
itself (`www/`) is a Vite + React app, the repo's first JS toolchain,
adopted because its table view uses Glide Data Grid (React-only) and its
components must also mount inside a Python anywidget later (which expects a
bundled JS asset anyway).

## Commands

`Makefile` wraps every command below; `make help` lists them. One-time
setup, then day-to-day work is just `make dev` / `make build` / `make test`.
From the repo root, run them as `make -C apps/triplex-web <target>`.

## Build

One-time setup: `make setup`, or by hand:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked
cd www && npm install
```

`wasm-bindgen-cli`'s version must exactly match the `wasm-bindgen` crate
version pinned in `Cargo.toml` (and in the `Makefile`). If that version
changes, reinstall the CLI to match.

Build the `.wasm` and generate its JS glue into `www/pkg/` (gitignored,
regenerate after every Rust source change): `make wasm`, or by hand:

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

For a production build: `make build`, then `make preview`, or
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

`.csv`/`.tsv`/`.jsonl` only: `.parquet` input isn't supported here. Its
codecs (zstd/snap) need a C toolchain that `wasm32-unknown-unknown` doesn't
have; see `crates/triplex-io/Cargo.toml` for the per-target dependency
split that keeps native builds (the CLI) unaffected.

Tier 1 ("Basic Anonymization"): one algorithm per column, picked from
whatever `triplex-anonymizer::AnonymizeMethod::ALL` currently implements
(just `token_hmac` today). Tier 2 ("Synthetic Data Generation") and Tier 3
("Pattern-persistent Data Generation") are not part of this build: their
cards link to Triplex Pro, a paid extension that adds both dialogs. Synthetic
data itself is open: `triplex synthesize` in the CLI and `triplex.synthesize`
in the Python package run the same engine.

## Generator extensions

Tiers 2 and 3 are generator extensions. Each one adds a dialog that produces
a new table from the loaded one, and a dataset tab for the result. The app
knows them only through the contract in `www/src/extensionApi.js`;
`www/src/extensions.js` lists the ones this build ships. Another build can
replace that list through a Vite alias on the specifier `./extensions`, the
same way the widget build swaps `./workers`. This build's list is empty, so
the tier cards for 2 and 3 become links, and the dataset tabs and the Compare
button, which only make sense next to a generated table, are hidden.
