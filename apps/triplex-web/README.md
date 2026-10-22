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

For the hosted demo: `make demo` builds the app into `www/dist-demo/`
(`www/vite.demo.config.js`, page `www/demo.html`, entry `www/src/demo.jsx`),
ready for a static host: `index.html`, the assets, and a `_headers` file.
See "Hosted demo" below.

`make test`/`make clippy`/`make fmt` run against this crate and everything
it depends on (`triplex-io`, `triplex-schemacast`, `triplex-anonymizer`).
`make clean` removes `www/pkg`, `www/dist`, `www/dist-widget`, `www/dist-demo`, and this crate's `target/`
output.

## Scope

`.csv`/`.tsv`/`.jsonl` only: `.parquet` input isn't supported here. Its
codecs (zstd/snap) need a C toolchain that `wasm32-unknown-unknown` doesn't
have; see `crates/triplex-io/Cargo.toml` for the per-target dependency
split that keeps native builds (the CLI) unaffected.

Tier 1 ("Basic Anonymization"): one method per column, picked from whatever
`triplex-anonymizer::AnonymizeMethod::ALL` implements (`token_hmac`, `fpe`,
`bucket`, `partial_mask` and `drop`). "Download recipe" saves the picks as a
`.triplex.yaml` recipe that `triplex anonymize` runs. "Open recipe", or a
`.yaml` file dropped on the page, applies a recipe to the loaded table, in
every build including the hosted demo: a recipe is configuration, not data.
The engine checks it the same way the command-line tool does. A recipe never
holds the salt, only the name of the environment variable the command-line
tool reads it from, so keyed methods still need the salt typed in. Settings
the column menu has no control for (a bucket width, the characters partial
masking keeps) are kept from the recipe and written back unchanged. Tier 2 ("Synthetic Data Generation") and Tier 3
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

## Hosted demo

A build of the same app for trying it without installing anything. It opens
on one of two synthetic datasets and has no way to load a data file of one's
own: the file chooser and the drop area are replaced by a dataset picker
(`datasetPicker` on `App`). A banner says so and links to the install page.
A recipe can still be opened there, since it is configuration, not data.

- **Datasets.** `tests/fixtures/demo/municipal.csv` (a municipal benefits
  register, 1,200 rows) and `statistical.csv` (person-level microdata, 2,000
  rows), generated from a fixed seed by `www/src/demo/generate.js`. After a
  change there, `npm run demo-data` rewrites both files; a test fails while
  they disagree. Other tests check what each dataset is for: in the register,
  postcode, birth year and sex single out most people only in combination,
  and grouping the birth year by decade removes most of that; in the
  microdata, income rises with education and rare combinations exist.
- **Links.** `#dataset=statistical` opens a given dataset, so a page can link
  straight to it. Links from the demo back to `triplexanalytics.io` carry
  `utm_source=demo` and a `utm_content` per link; the demo itself counts
  nothing.
- **What the page may contact.** A content security policy
  (`www/src/demo/contentSecurityPolicy.js`) allows only the demo's own origin,
  so the browser blocks any request elsewhere. It is added to the built page as
  a `<meta>` tag and written to `dist-demo/_headers` as a response header, with
  framing forbidden and `noindex`. The dev server (`npm run dev:demo`) runs
  without it, because it injects an inline script of its own.
- **Small screens.** Below 900 pixels wide the app does not start; the page
  says the demo needs a larger screen.
- **Extensions.** `src/demo.jsx` imports `./extensions` like `App.jsx` does,
  so a build that swaps the list gets it in the demo too. With extensions
  present, the banner and footer say that Tiers 2 and 3 are Triplex Pro.
