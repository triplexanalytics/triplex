# triplex-mcp

MCP (Model Context Protocol) server over SchemaCast, stdio transport only —
see the design notes. A coding agent (Claude Code or any other MCP client)
launches this as a local process and talks to it over its own stdin/stdout,
same trust boundary as `apps/triplex-cli`: no network, no server, nothing
here reads bytes over the wire.

## Tools

- `schema_infer(path, sample_rows?)` — `triplex_schemacast::infer_schema_from_path`
- `schema_validate(schema_path, input_path, sample_rows?)` — `triplex_schemacast::validate_path`

Both are read-only and take a local file path. `anonymize` isn't exposed
yet.

## Run

```sh
cargo run -p triplex-mcp
```

Speaks newline-delimited JSON-RPC 2.0 on stdin/stdout — point an MCP
client's stdio transport at this binary. No flags, no config file.

## Build/test

Ordinary workspace crate: `cargo build -p triplex-mcp`, `cargo test -p
triplex-mcp`, `cargo clippy -p triplex-mcp --all-targets`. Covered by the
root `cargo test --workspace`/CI already — no separate Makefile target.
