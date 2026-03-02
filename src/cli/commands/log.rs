// ABOUTME: `pasua log` command — per-commit mini-overview for a ref range.
// ABOUTME: Walks each commit and produces a Layer 1 overview per commit.

use clap::Args;
use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

use crate::core::{pipeline, render};

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
    let commits = list_commits(repo, &args.range)?;

    for (sha, subject) in &commits {
        let parent = format!("{sha}^");
        let result = pipeline::run(repo, &parent, sha, args.threshold, false).await?;

        // Mini header: sha "subject"  +add/-del  Nf
        println!(
            "{} \"{}\"  +{}/−{}  {}f",
            &sha[..7],
            subject,
            result.summary.total_added,
            result.summary.total_removed,
            result.summary.file_count,
        );

        // Render file lines only (no header line)
        for file in &result.files {
            let line = render::file_line_only(file);
            println!("  {line}");
        }
        println!();
    }

    Ok(())
}

/// Return list of (sha, subject) for commits in the given range, oldest first.
fn list_commits(repo: &PathBuf, range: &str) -> Result<Vec<(String, String)>> {
    let output = Command::new("git")
        .args(["log", "--reverse", "--format=%H %s", range])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git log failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut commits = Vec::new();
    for line in String::from_utf8(output.stdout)?.lines() {
        if let Some((sha, subject)) = line.split_once(' ') {
            commits.push((sha.to_string(), subject.to_string()));
        }
    }
    Ok(commits)
}
