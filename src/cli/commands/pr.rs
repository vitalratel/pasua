// ABOUTME: `pasua pr` command — PR envelope with metadata + Layer 1 diff.
// ABOUTME: Fetches PR title, description, CI status via gh CLI.

use clap::Args;
use anyhow::Result;

#[derive(Args, Debug)]
pub struct PrArgs {
    /// Path to local repository clone
    pub repo: String,
    /// Pull request number
    pub number: u64,
}

pub async fn run(_args: PrArgs) -> Result<()> {
    todo!("pr command")
}
