# Changelog

Notable changes to Triplex. The format follows [Keep a Changelog](https://keepachangelog.com/),
and versions follow [Semantic Versioning](https://semver.org/). Versions are git tags:
`workspace-v*` for the engine and CLI, `web-v*` for the web UI.

## [Unreleased]

### Added

- **Sketch steering in the web UI.** In the Sketch dialog a class can be split in two along a
  column, its spread on a number column redrawn by dragging across the bars, and a new class
  added by hand. A downloaded sketch can be loaded back.
- **Python package.** `import triplex` runs the engine in plain Python, with no browser and no
  subprocess: `infer_schema`, `validate`, `convert`, `anonymize` and `synthesize`. Tables go in
  as a file path, a pandas or Polars DataFrame or a pyarrow Table. Built from source for now; not
  on PyPI yet.
- **Docker image.** A `Dockerfile` that builds the `triplex` CLI into a small image running as a
  non-root user. It works with `--network none`.

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
