// ABOUTME: `pasua log` command — per-commit mini-overview for a ref range.
// ABOUTME: Walks each commit and produces a Layer 1 overview per commit.

use clap::Args;
use anyhow::Result;
use std::path::PathBuf;

use crate::core::{github, pipeline, render};

#[derive(Args, Debug)]
pub struct LogArgs {
    /// Path to local repository clone
    pub repo: PathBuf,
    /// Commit range, e.g. main..feature
    pub range: String,
    /// Line delta threshold for auto Layer 2 (default: 200)
    #[arg(long, default_value = "200")]
    pub threshold: usize,
}

pub async fn run(args: LogArgs) -> Result<()> {
    let repo = &args.repo;
    let commits = github::list_commits(repo, &args.range)?;

    for (sha, subject) in &commits {
        let parent = format!("{sha}^");
        let result = pipeline::run(repo, &parent, sha, args.threshold, false).await?;

        println!(
            "{} \"{}\"  +{}/−{}  {}f",
            &sha[..7],
            subject,
            result.summary.total_added,
            result.summary.total_removed,
            result.summary.file_count,
        );

        for file in &result.files {
            let line = render::file_line_only(file);
            println!("  {line}");
        }
        println!();
    }

    Ok(())
}
