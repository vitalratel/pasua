// ABOUTME: `pasua hunk` command — Layer 3 scoped diff for a single symbol.
// ABOUTME: Returns unified diff lines scoped to the symbol's line range only.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::core::hunk;

#[derive(Args, Debug)]
pub struct HunkArgs {
    /// Path to local repository clone
    pub repo: PathBuf,
    /// Base ref (branch, commit, or tag)
    pub base: String,
    /// Head ref (branch, commit, or tag)
    pub head: String,
    /// File path (relative to repo root)
    pub file: String,
    /// Symbol name
    pub symbol: String,
}

pub async fn run(args: HunkArgs) -> Result<()> {
    let output = hunk::symbol_hunk(&args.repo, &args.base, &args.head, &args.file, &args.symbol)?;
    print!("{output}");
    Ok(())
}
