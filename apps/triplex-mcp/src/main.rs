//! MCP (Model Context Protocol) surface over SchemaCast, stdio transport.
//! Mirrors `apps/triplex-cli`'s role: owns tool-facing signatures and error
//! conversion only, no logic of its own — everything real lives in
//! `triplex-schemacast`. Same trust boundary as the CLI: an MCP stdio
//! server is a local process an agent launches directly, not a network
//! service, so it stays inside the local-first/client-perimeter rule.
//!
//! Two tools today — `schema_infer`/`schema_validate`, the two read-only
//! SchemaCast operations. `anonymize` deliberately isn't exposed yet: it
//! touches salts/mapping files, a bigger surface to get right, and lands
//! as its own follow-up once this minimal slice is proven. A local file
//! path is the boundary for both tools, same as the CLI: nothing here
//! reads bytes over the wire.

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
    transport::io::stdio,
    ErrorData, ServerHandler, ServiceExt,
};
use std::path::PathBuf;

const SAMPLE_ROWS: usize = 1000;

fn to_mcp_err<E: std::fmt::Display>(e: E) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SchemaInferRequest {
    #[schemars(description = "local path to a .csv/.tsv/.jsonl/.parquet file")]
    path: String,
    #[schemars(
        description = "rows to sample for type inference (ignored for .parquet, which \
        reads its own footer schema); defaults to 1000"
    )]
    sample_rows: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SchemaValidateRequest {
    #[schemars(description = "local path to a declared .triplex.yaml contract")]
    schema_path: String,
    #[schemars(description = "local path to the .csv/.tsv/.jsonl/.parquet file to validate")]
    input_path: String,
    #[schemars(description = "rows to sample for inference before validating; defaults to 1000")]
    sample_rows: Option<u32>,
}

#[derive(Debug, Clone)]
struct TriplexMcp {
    tool_router: ToolRouter<Self>,
}

impl TriplexMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl TriplexMcp {
    #[tool(
        description = "Infer a versioned schema contract (column types, null rates, \
        semantic-type hints, distribution) from a local data file."
    )]
    fn schema_infer(
        &self,
        Parameters(req): Parameters<SchemaInferRequest>,
    ) -> Result<String, ErrorData> {
        let sample_rows = req.sample_rows.map(|n| n as usize).unwrap_or(SAMPLE_ROWS);
        let contract =
            triplex_schemacast::infer_schema_from_path(&PathBuf::from(req.path), sample_rows)
                .map_err(to_mcp_err)?;
        serde_json::to_string_pretty(&contract).map_err(to_mcp_err)
    }

    #[tool(
        description = "Validate a local data file's inferred schema against a declared \
        .triplex.yaml contract — presence, type, and any declared statistical thresholds."
    )]
    fn schema_validate(
        &self,
        Parameters(req): Parameters<SchemaValidateRequest>,
    ) -> Result<String, ErrorData> {
        let sample_rows = req.sample_rows.map(|n| n as usize).unwrap_or(SAMPLE_ROWS);
        let report = triplex_schemacast::validate_path(
            &PathBuf::from(req.schema_path),
            &PathBuf::from(req.input_path),
            sample_rows,
        )
        .map_err(to_mcp_err)?;
        serde_json::to_string_pretty(&report).map_err(to_mcp_err)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TriplexMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Triplex SchemaCast tools: infer and validate schema contracts for local data \
             files. Local-first — every tool reads only the local path it's given, never the \
             network.",
        )
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = TriplexMcp::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
