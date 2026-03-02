// ABOUTME: `pasua serve` command — starts the MCP server on stdio.
// ABOUTME: Exposes the same operations as the CLI via MCP protocol.

use clap::Args;
use anyhow::Result;

#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Log level: error, warn, info, debug, trace (env: PASUA_LOG_LEVEL)
    #[arg(long, env = "PASUA_LOG_LEVEL", default_value = "info")]
    pub log_level: String,
}

pub async fn run(_args: ServeArgs) -> Result<()> {
    todo!("serve command")
}
