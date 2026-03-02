// ABOUTME: `pasua diff` command — Layer 1 overview with optional auto-Layer 2.
// ABOUTME: Auto-includes Layer 2 for split files and files exceeding the line threshold.

use clap::Args;
use anyhow::Result;
use std::path::PathBuf;

use crate::core::{github, pipeline, render};

#[derive(Args, Debug)]
pub struct DiffArgs {
    /// Path to local repository clone
    pub repo: PathBuf,
    /// Base ref (branch, commit, or tag)
    pub base: String,
    /// Head ref (branch, commit, or tag)
    pub head: String,
    /// Include Layer 2 symbols for all files
    #[arg(long = "depth", value_name = "DEPTH")]
    pub depth_symbols: bool,
    /// Line delta threshold for auto-including Layer 2 (default: 200)
    #[arg(long, default_value = "200")]
    pub threshold: usize,
}

pub async fn run(args: DiffArgs) -> Result<()> {
    let repo = &args.repo;
    let result = pipeline::run(repo, &args.base, &args.head, args.threshold)?;
    let repo_label = github::remote_name(repo).unwrap_or_else(|_| repo.display().to_string());
    let output = render::layer1(&result, &repo_label, &args.base, &args.head);
    print!("{output}");
    Ok(())
}
