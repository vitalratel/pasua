// ABOUTME: `pasua log` command — file-level overview for each commit in a range.
// ABOUTME: Walks each commit and produces a file-level diff overview.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::core::{git, pipeline, render};

#[derive(Args, Debug)]
pub struct LogArgs {
    /// Path to local repository clone
    pub repo: PathBuf,
    /// Commit range, e.g. main..feature
    pub range: String,
    /// Line delta threshold for auto-expanding a file's symbols
    #[arg(long, default_value = "200")]
    pub threshold: usize,
}

pub async fn run(args: LogArgs) -> Result<()> {
    let repo = &args.repo;
    let commits = git::list_commits(repo, &args.range)?;

    for (sha, subject) in &commits {
        let parent = format!("{sha}^");
        let result = pipeline::run(repo, &parent, sha, args.threshold, false, true).await?;

        println!("{}", render::log_entry(sha, subject, &result));

        for file in &result.files {
            let line = render::file_line_only(file);
            println!("  {line}");
        }
        println!();
    }

    Ok(())
}
