// ABOUTME: `pasua serve` command — starts the MCP server on stdio.
// ABOUTME: Exposes the same operations as the CLI via MCP protocol.

use anyhow::{Context, Result};
use clap::Args;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

use crate::mcp::PasuaServer;

#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Log level: error, warn, info, debug, trace (env: PASUA_LOG_LEVEL)
    #[arg(long, env = "PASUA_LOG_LEVEL", default_value = "info")]
    pub log_level: String,
}

pub async fn run(args: ServeArgs) -> Result<()> {
    init_tracing(&args.log_level);

    tracing::info!("starting pasua MCP server on stdio");

    let transport = rmcp::transport::stdio();
    let server = PasuaServer::new();
    let running = server
        .serve(transport)
        .await
        .context("Failed to start MCP server")?;

    tracing::info!("MCP server running");
    running.waiting().await?;

    Ok(())
}

fn init_tracing(level: &str) {
    let filter = match std::env::var("RUST_LOG") {
        Ok(val) if !val.is_empty() => EnvFilter::new(val),
        _ => EnvFilter::new(format!("pasua={level}")),
    };

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();
}
