# Working in this repository

This file is for coding agents. Human contributors: start with [CONTRIBUTING.md](CONTRIBUTING.md).

## What Triplex is

Local-first data privacy tooling. One Rust engine with Arrow-compatible data contracts, delivered
as a command-line tool, a WebAssembly web UI, Python bindings, a Jupyter widget and a local MCP
server. The products built on the engine are SchemaCast (schema contracts and conversion),
Anonymizer (pseudonymization and synthetic data), DatasetCard (privacy-risk scoring), DriftGuard
(schema and drift checks) and ModelCard (fairness and slice metrics).

## The boundary that never moves

Triplex runs inside the user's own perimeter. A user's data, identifiers, mapping files, salts,
passphrases and profiling outputs never leave the machine or the browser tab.

- Add no network request, telemetry, analytics, crash reporting, update check or cloud
  dependency. This includes the web UI, build scripts that end up in a shipped artifact, and
  generated reports (they must open offline, with no external script, font or stylesheet).
- If a task seems to need one, stop and say so. Do not look for a way around it.

## Layout

| Path | Contents |
|---|---|
| `crates/triplex-core`, `crates/triplex-io` | Shared platform: data contracts and file formats |
| `crates/triplex-schemacast`, `-anonymizer`, `-datasetcard`, `-driftguard`, `-modelcard` | One crate per product |
| `apps/triplex-cli` | The `triplex` command-line tool |
| `apps/triplex-web` | WebAssembly bindings (`src/`) and the React UI (`www/`) |
| `apps/triplex-mcp` | Local MCP server over SchemaCast, stdio only |
| `packages/triplex-python` | Python bindings (PyO3) |
| `packages/triplex-jupyter` | The web UI as a Jupyter widget |
| `tests/fixtures/` | Synthetic test data |

Configuration is a version-controlled `.triplex.yaml` file and nothing else. Commands that
produce a report write three files: a deterministic audit JSON, a standalone offline HTML report
and a Python script that reproduces the numbers with Polars.

## Before you edit

1. Read the code you are about to change and, where there is one, the README of its app or package.
2. State the objective, the files you expect to touch, the tests you will add or run, and what
   you are unsure about.
3. Stop and ask before changing scope, security behavior, licensing, a public API, a file format
   or anything that touches the boundary above.

Requirements come from an issue or from the maintainer. Do not invent product requirements.

## While you edit

- Make the smallest coherent change that works end to end.
- Prefer deterministic behavior and typed, versioned contracts.
- Extend the platform through traits and structs with clear boundaries, so a new product or
  format does not require reworking existing call sites.
- Add tests. Use synthetic data only, from `tests/fixtures/` or generated in the test. Never put
  real personal data, a real mapping file or a credential into a test, a fixture, a commit, a log
  or a prompt.
- A statement about privacy, anonymization strength, fairness or compliance needs a test that
  demonstrates it. If there is no such test, do not make the statement: not in code, a report,
  the UI, a README or the changelog. Tests can support a differential-privacy guarantee but
  cannot prove one, so describe it accordingly.
- A metric that cannot be computed from the data at hand is an error, never an estimated or
  placeholder number.

## Before you finish

Run what applies to your change and report the exact commands and their results. If something
fails or was skipped, say so.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Web UI (apps/triplex-web/www)
npm run lint && npm run typecheck && npm run format:check && npm test

# Python bindings (packages/triplex-python)
pip install ".[dev]" && python -m pytest tests
```

Then:

- Add an entry under "Unreleased" in [CHANGELOG.md](CHANGELOG.md) for any change a user or an
  API consumer can notice.
- Update the README of the crate, app or package when its behavior or usage changed.
- Summarize the change file by file, and name the risks that remain.

## Writing

Everything in this repository is public and should read as finished work.

- The repository stands on its own. Do not cite documents, decision records, ticket numbers or
  discussions that a reader of this repository cannot open. Explain the reasoning itself.
- Comments explain why the code is the way it is. They do not narrate history.
- No em dashes in text a user sees: UI copy, CLI output, error messages, generated reports,
  READMEs and the changelog. Use a period, comma, colon or parentheses.
- No placeholders, no "TODO" without an issue, no claims about features that do not exist yet.

## Git and pull requests

- Work on a focused branch: `feat/*`, `fix/*`, `docs/*` or `chore/*`. Never commit to `main`.
- Do not commit or push unless you are asked to. Do not merge: the maintainer merges.
- Give every pull request a conventional-commit title, for example
  `feat(web): compare dialog` or `fix(io): reject ragged rows`. Pull requests are squashed, so
  the title becomes the commit on `main`, and it decides the version.
- Versions are git tags created by CI on every merge to `main`: `workspace-v*` for the engine
  and CLI, `web-v*` for the web UI, `python-v*` and `jupyter-v*` for the two Python packages. A
  `feat` title or a breaking change (`!`) raises the minor version, anything else the patch
  version. Never edit a `version` field by hand; `Cargo.toml`, `package.json` and
  `pyproject.toml` hold a `0.0.0` placeholder on purpose.
- Keep engine changes (Rust: `crates/`, `apps/*/src`) and UI changes (`apps/*/www`, Python UI
  code) in separate pull requests. When a UI change needs new engine surface, open the engine
  pull request first and stack the UI one on top of it.
- Commits and pull requests carry no tool-attribution trailers (`Co-Authored-By`,
  `Generated with` and the like).

## Security

Report vulnerabilities privately, as described in [SECURITY.md](SECURITY.md). An unexpected
network request from any Triplex component is a vulnerability.
