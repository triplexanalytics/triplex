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

- **Extension slot in the web UI.** Tiers 2 and 3 are generator extensions behind one contract,
  chosen at build time, so another build can add dialogs without changing the app.
- **Python package.** `import triplex` runs the engine in plain Python, with no browser and no
  subprocess: `infer_schema`, `validate`, `convert`, `anonymize` and `synthesize`. Tables go in
  as a file path, a pandas or Polars DataFrame or a pyarrow Table. Built from source for now; not
  on PyPI yet. `draft_contract` returns a starting contract for a table, and `validate` takes a
  contract as a `.triplex.yaml` path or as a dict.
- **Docker image.** A `Dockerfile` that builds the `triplex` CLI into a small image running as a
  non-root user. It works with `--network none`.

### Changed

- **The command-line tool's Cargo package is now called `triplex`**, so it can be installed with
  `cargo install triplex` once published. Build it with `cargo build --release -p triplex`; the
  binary is still `triplex`. Every crate now carries the metadata crates.io needs, and
  `triplex --version` reports the published version when built from crates.io.

### Fixed

- **Jupyter widget.** Dialogs are centered in and sized to the widget's own box, for notebook
  hosts such as VS Code where the cell is not the viewport.

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
