// ABOUTME: `pasua hunk` command — Layer 3 scoped diff for a single symbol.
// ABOUTME: Returns a unified diff scoped to the symbol's lines only.

use clap::Args;
use anyhow::Result;

#[derive(Args, Debug)]
pub struct HunkArgs {
    /// Path to local repository clone
    pub repo: String,
    /// Base ref
    pub base: String,
    /// Head ref
    pub head: String,
    /// File path (relative to repo root)
    pub file: String,
    /// Symbol name
    pub symbol: String,
}

pub async fn run(_args: HunkArgs) -> Result<()> {
    todo!("hunk command")
}
