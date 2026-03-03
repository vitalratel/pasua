// ABOUTME: Entry point for pasua — semantic code diff tool for AI agents.
// ABOUTME: Dispatches to CLI commands or MCP server mode.

mod cli;
mod core;
mod languages;
mod mcp;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let cli = cli::Cli::parse();
    cli::run(cli).await
}
