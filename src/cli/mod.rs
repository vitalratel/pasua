// ABOUTME: CLI module — command definitions and dispatch.
// ABOUTME: All commands share the same core library functions.

mod commands;

pub use commands::{Cli, Commands};

use anyhow::Result;

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Diff(args) => commands::diff::run(args).await,
        Commands::Symbols(args) => commands::symbols::run(args).await,
        Commands::Hunk(args) => commands::hunk::run(args).await,
        Commands::Pr(args) => commands::pr::run(args).await,
        Commands::Log(args) => commands::log::run(args).await,
        Commands::Serve(args) => commands::serve::run(args).await,
    }
}
