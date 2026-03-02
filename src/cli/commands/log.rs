// ABOUTME: `pasua log` command — per-commit mini-overview for a ref range.
// ABOUTME: Walks each commit individually for incremental history.

use clap::Args;
use anyhow::Result;

#[derive(Args, Debug)]
pub struct LogArgs {
    /// Path to local repository clone
    pub repo: String,
    /// Commit range, e.g. main..feature
    pub range: String,
}

pub async fn run(_args: LogArgs) -> Result<()> {
    todo!("log command")
}
