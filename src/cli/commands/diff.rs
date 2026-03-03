// ABOUTME: `pasua diff` command — Layer 1 overview with optional auto-Layer 2.
// ABOUTME: Auto-includes Layer 2 for split files and files exceeding the line threshold.

use anyhow::Result;
use clap::Args;
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
    /// Symbol expansion: symbols = force all, none = suppress all [default: auto]
    #[arg(long, value_name = "DEPTH")]
    pub depth: Option<String>,
    /// Line delta threshold for auto-expanding a file's symbols
    #[arg(long, default_value = "200")]
    pub threshold: usize,
}

pub async fn run(args: DiffArgs) -> Result<()> {
    let repo = &args.repo;
    let depth_symbols = args.depth.as_deref() == Some("symbols");
    let expand = args.depth.as_deref() != Some("none");
    let result = pipeline::run(
        repo,
        &args.base,
        &args.head,
        args.threshold,
        depth_symbols,
        expand,
    )
    .await?;
    let repo_label = github::remote_name(repo, &args.base, &args.head)
        .unwrap_or_else(|_| repo.display().to_string());
    let output = render::layer1(&result, &repo_label, &args.base, &args.head);
    print!("{output}");
    Ok(())
}
