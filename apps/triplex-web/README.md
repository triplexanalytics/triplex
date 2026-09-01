# triplex-web

Minimal browser UI over SchemaCast/Anonymizer — see this repo's S3.5 step in
the design notes. Static HTML + vanilla JS,
no bundler, no server: everything runs client-side via WebAssembly.

## Build

One-time setup:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked
```

`wasm-bindgen-cli`'s version must exactly match the `wasm-bindgen` crate
version pinned in `Cargo.toml` — if that version changes, reinstall the CLI
to match.

Build the `.wasm` and generate its JS glue into `www/pkg/` (gitignored,
regenerate after every source change):

```sh
cargo build -p triplex-web --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir www/pkg --no-typescript \
  ../../target/wasm32-unknown-unknown/release/triplex_web.wasm
```

## Run

```sh
cd www && python3 -m http.server 8080
```

Open `http://localhost:8080/index.html`. A browser won't load `.wasm` over
`file://` (CORS/MIME restrictions), so it needs a real (even if trivially
local) HTTP server — no build step or bundler beyond the two commands above.

## Scope

`.csv`/`.tsv`/`.jsonl` only — `.parquet` input isn't supported here. Its
codecs (zstd/snap) need a C toolchain that `wasm32-unknown-unknown` doesn't
have; see `crates/triplex-io/Cargo.toml` for the per-target dependency
split that keeps native builds (the CLI) unaffected.
