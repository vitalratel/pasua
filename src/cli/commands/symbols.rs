// ABOUTME: `pasua symbols` command — Layer 2 symbol table for one file.
// ABOUTME: Shows where each symbol moved or how it changed.

use clap::Args;
use anyhow::Result;

#[derive(Args, Debug)]
pub struct SymbolsArgs {
    /// Path to local repository clone
    pub repo: String,
    /// Base ref
    pub base: String,
    /// Head ref
    pub head: String,
    /// File path (relative to repo root)
    pub file: String,
}

pub async fn run(_args: SymbolsArgs) -> Result<()> {
    todo!("symbols command")
}
