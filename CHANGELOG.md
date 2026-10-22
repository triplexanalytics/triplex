# Changelog

Notable changes to Triplex. The format follows [Keep a Changelog](https://keepachangelog.com/),
and versions follow [Semantic Versioning](https://semver.org/). Versions are git tags:
`workspace-v*` for the engine and CLI, `web-v*` for the web UI, `python-v*` for the Python package
and `jupyter-v*` for the Jupyter widget.

## [Unreleased]

### Removed

- **Synthesize and Sketch dialogs in the web UI and the Jupyter widget.** They move to Triplex
  Pro, a paid extension; their tier cards now link to where it is described. Synthetic data
  stays open source in the engine, the `triplex synthesize` command and the Python package.
  Versions of the web UI released before this change remain available under Apache-2.0.

### Added

- **A demo build of the web UI.** `make -C apps/triplex-web demo` builds the app for a static
  host: it opens on one of two synthetic sample datasets (a municipal benefits register and
  person-level microdata, in `tests/fixtures/demo/`) and cannot load a file of one's own. A
  content security policy limits the page to its own origin, so the browser blocks requests
  anywhere else. Hosts that embed the app can replace the file chooser with their own control
  (`datasetPicker`).
- **`triplex risk`.** Prints, as JSON, how well a table's quasi-identifiers (an age, a postcode,
  a plan) hide each person: k-anonymity for each column alone, for every pair and for all of them
  together, how many rows no other row shares all values with, and the rows most at risk (SUDA,
  or an explainable boosting model with `--rows ebm`). Reads every row. Without `--quasi` it
  chooses the columns and lists the ones it left out and why. The same report is available to
  Rust callers as `triplex_datasetcard::assess`.
- **Extension slot in the web UI.** Tiers 2 and 3 are generator extensions behind one contract,
  chosen at build time, so another build can add dialogs without changing the app.
- **`triplex_anonymizer::recipe_from_yaml`.** Reads and checks recipe text the same way
  `triplex anonymize` reads a recipe file, for callers that hold the text instead of a path.

### Changed

- **Recipes are checked when they are loaded.** `triplex anonymize`, the Python package and
  `triplex_anonymizer::load_recipe` now refuse a recipe that has no `version` or a version other
  than 1, a column setting they do not recognize (a misspelled `bucket_width` used to be ignored
  and the default used), a `salt` key in a column (a recipe names the variable that holds the
  salt, never the salt), a keyed method (`token_hmac`, `fpe`) without `salt_env`, no columns, or
  a column declared twice (the second entry used to be ignored). The message names the column.
  `verify_recipe_well_formed` checks the version and `salt_env` as well.

### Fixed

- **Loading a second file in the web UI no longer blanks the page.** The table kept state from
  the first file and failed when the new one had different columns.
- **The smallest group in a k-anonymity result is the same on every run.** When several groups
  tie for smallest, the one whose first row comes earliest is named. Before, the choice could
  change between runs of the same table.
- **A recipe needs `salt_env` only for keyed methods.** Bucketing and partial masking run
  without a salt and without an environment variable; `token_hmac` and `fpe` without
  `salt_env` stop with an error that names the column.
- **Recipe and contract files carry only what is set.** A written recipe lists only the
  settings its method uses, and a drafted contract no longer contains `null` entries. Files
  written the old way still read.
- **A recipe downloaded from the web UI runs in the command line.** Keyed methods name
  `TRIPLEX_SALT` as their salt variable (export it with the salt you typed in the browser to
  get the same tokens); other methods name none.
- **The web UI offers the mapping file.** After a masking run, "Download data" lists
  "Mapping file (.csv)", set apart from the data formats, so the masking can be reversed.
- **Menus stay inside the window.** A column's menu opened near the right or bottom edge moves
  back into view, and the "Download data" list opens above the side panel.
- **Web UI navbar before a file is loaded.** The Basic Anonymization tier is now disabled
  along with the download buttons until there is a file to work on.
- **License label in the web UI.** The navbar shows "License: Free" in the open build and
  "License: Pro" in Triplex Pro, instead of always "License: Pro".
- **Jupyter widget.** Dialogs are centered in and sized to the widget's own box, for notebook
  hosts such as VS Code where the cell is not the viewport.

## Engine and CLI 0.43.1 - 2026-10-17

The first release on crates.io and of the container image.

### Added

- **crates.io.** `cargo install triplex` installs the command-line tool. The engine crates are
  published alongside it (`triplex-core`, `triplex-io`, `triplex-schemacast`,
  `triplex-anonymizer`, `triplex-datasetcard`, `triplex-driftguard`, `triplex-modelcard`), and
  `triplex-mcp` is the local MCP server. `triplex --version` reports the published version.
- **Docker image.** `ghcr.io/triplexanalytics/triplex`, for amd64 and arm64: the `triplex` CLI
  in a small image that runs as a non-root user and works with `--network none`. The
  `Dockerfile` builds the same image locally.

### Changed

- **The command-line tool's Cargo package is called `triplex`**, matching the binary. From a
  checkout, build it with `cargo build --release -p triplex`.

## Python package 0.2.2 - 2026-10-17

The first release on PyPI: `pip install triplexanalytics`, then `import triplex`.

### Added

- **Python package.** Runs the engine in plain Python, with no browser and no subprocess:
  `infer_schema`, `draft_contract`, `validate`, `convert`, `anonymize` and `synthesize`. Tables
  go in as a file path, a pandas or Polars DataFrame or a pyarrow Table and come back as a
  `Table` (`to_pandas()`, `to_polars()`, `to_arrow()`). `validate` takes a contract as a
  `.triplex.yaml` path or as a dict. Wheels for Linux (x86_64 and aarch64), macOS and Windows,
  one per platform for Python 3.9 and later.

## First public release

### Added

- **SchemaCast.** Schema inference for CSV, TSV, JSON Lines and Parquet. `.triplex.yaml`
  contracts with validation, range and null-rate thresholds, pattern checks and rule-based
  detection of sensitive columns. Conversion between formats.
- **Anonymizer, Tier 1.** Keyed HMAC tokens, format-preserving substitution, numeric bucketing,
  partial masking and column removal. The mapping file is written separately and can be
  encrypted with a passphrase.
- **Anonymizer, Tier 2.** Synthetic data from a Gaussian copula, a discrete Bayesian network or
  independent marginals, each with a fidelity report and a measured leakage report. An optional
  differentially private mode with a required epsilon and a budget accountant, awaiting
  independent review.
- **Anonymizer, Tier 3.** Sketch-based synthesis: propose a sketch from the data, steer it on
  parallel coordinates and generate from it. No privacy guarantee.
- **DatasetCard.** k-anonymity, l-diversity, t-closeness, beta-likeness and delta-disclosure per
  column and per combined quasi-identifier, plus per-record risk scores.
- **DriftGuard.** Schema-regression and statistical-drift checks between a baseline and a
  candidate, with exit codes built for CI.
- **ModelCard.** Per-slice performance and group-fairness metrics for a binary classifier, from a
  predictions file.
- **Reports.** Commands that produce a report write a deterministic audit JSON, a standalone
  offline HTML report and a Python script that reproduces the numbers with Polars.
- **Web UI.** The engine compiled to WebAssembly: table view, per-column anonymization, risk
  analysis and synthesis, all inside the browser tab.
- **Jupyter widget.** The web UI inside a notebook cell, with results readable from Python.
- **MCP server.** Schema inference and validation as local tools for coding agents, over stdio.
