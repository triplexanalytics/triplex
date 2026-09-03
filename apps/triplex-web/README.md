# triplex-web

Browser UI over SchemaCast/Anonymizer — see this repo's S3.5 step in
the design notes. Everything still runs
client-side via WebAssembly: no server, no upload, no telemetry. The UI
itself (`www/`) is a Vite + React app — the repo's first JS toolchain,
adopted because its table view uses Glide Data Grid (React-only) and its
components must also mount inside a Python anywidget later (which expects a
bundled JS asset anyway); see the design notes and
the design notes.

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

`make test`/`make clippy`/`make fmt` run against this crate and everything
it depends on (`triplex-io`, `triplex-schemacast`, `triplex-anonymizer`).
`make clean` removes `www/pkg`, `www/dist`, and this crate's `target/`
output.

## Scope

`.csv`/`.tsv`/`.jsonl` only — `.parquet` input isn't supported here. Its
codecs (zstd/snap) need a C toolchain that `wasm32-unknown-unknown` doesn't
have; see `crates/triplex-io/Cargo.toml` for the per-target dependency
split that keeps native builds (the CLI) unaffected.

Tier 1 ("Basic Anonymization") only — one algorithm per column, picked from
whatever `triplex-anonymizer::AnonymizeMethod::ALL` currently implements
(just `token_hmac` today). Tier 2/3 are shown in the tier picker but
disabled; see the design notes for what's deferred.
