// ABOUTME: `pasua diff` command — Layer 1 overview with optional Layer 2.
// ABOUTME: Auto-includes Layer 2 for split files and large modifications.

use clap::Args;
use anyhow::Result;

#[derive(Args, Debug)]
pub struct DiffArgs {
    /// Path to local repository clone
    pub repo: String,
    /// Base ref (branch, commit, or tag)
    pub base: String,
    /// Head ref (branch, commit, or tag)
    pub head: String,
    /// Include Layer 2 symbols for all files
    #[arg(long)]
    pub depth_symbols: bool,
    /// Line delta threshold for auto-including Layer 2 (default: 200)
    #[arg(long, default_value = "200")]
    pub threshold: usize,
}

pub async fn run(_args: DiffArgs) -> Result<()> {
    todo!("diff command")
}
