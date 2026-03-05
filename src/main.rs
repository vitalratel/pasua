// ABOUTME: Entry point for pasua — semantic code diff tool for AI agents.
// ABOUTME: Dispatches to CLI commands or MCP server mode.

mod cli;
mod core;
mod languages;
mod mcp;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    cli::run(cli).await
}
