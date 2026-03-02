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
    /// Layer 1 overview (+ auto Layer 2 for split/large files)
    Diff(diff::DiffArgs),
    /// Layer 2 symbol table for one file
    Symbols(symbols::SymbolsArgs),
    /// Layer 3 scoped diff for one symbol
    Hunk(hunk::HunkArgs),
    /// PR envelope + Layer 1
    Pr(pr::PrArgs),
    /// Per-commit mini-overview for a commit range
    Log(log::LogArgs),
    /// Start MCP server
    Serve(serve::ServeArgs),
}
