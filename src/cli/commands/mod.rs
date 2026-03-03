// ABOUTME: Command definitions for the pasua CLI.
// ABOUTME: Each subcommand maps to a module with its run() function.

pub mod diff;
pub mod hunk;
pub mod log;
pub mod pr;
pub mod serve;
pub mod symbols;

use clap::{Parser, Subcommand};

/// Token-efficient semantic code diff for AI coding agents.
#[derive(Parser, Debug)]
#[command(name = "pasua", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// File-level diff overview; auto-expands large and split files
    Diff(diff::DiffArgs),
    /// Changed symbols in one file
    Symbols(symbols::SymbolsArgs),
    /// Exact diff for one symbol
    Hunk(hunk::HunkArgs),
    /// PR metadata (title, CI status, reviews) with file-level diff
    Pr(pr::PrArgs),
    /// File-level overview for each commit in a range
    Log(log::LogArgs),
    /// Start MCP server
    Serve(serve::ServeArgs),
}
