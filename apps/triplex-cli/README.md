# triplex

The Triplex command-line tool: schema contracts, pseudonymization, synthetic data, privacy-risk
scoring and drift checks on local files. It makes no network request; your data stays where you
run it.

```sh
cargo install triplex
triplex --version
```

```sh
# Infer a schema from a file
triplex schema data.csv

# Validate a file against a contract (exit code 0 on a match, 2 on a breach)
triplex validate --schema data.contract.yaml --input data.csv

# Compare a candidate file against a baseline
# (exit code 0 for no drift, 1 for statistical drift, 2 for a schema regression)
triplex driftguard --baseline old.csv --candidate new.csv --out-dir drift
```

Run `triplex` without arguments for the full command list, including `convert`, `anonymize`,
`check-patterns`, `synthesize`, `modelcard` and `reveal-mapping`. Commands that produce a report
write three files: a deterministic audit JSON, a standalone offline HTML report and a Python
script that reproduces the numbers with Polars.

Source, Docker image and the other ways to run Triplex:
[github.com/triplexanalytics/triplex](https://github.com/triplexanalytics/triplex). Guides and
the glossary: [triplexanalytics.io/docs](https://triplexanalytics.io/docs/).

Licensed under [Apache-2.0](https://github.com/triplexanalytics/triplex/blob/main/LICENSE).
