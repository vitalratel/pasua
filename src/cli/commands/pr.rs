// ABOUTME: `pasua pr` command — PR envelope with metadata + Layer 1 diff.
// ABOUTME: Fetches PR title, description, CI status via gh CLI.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::core::{github, pipeline, render};

#[derive(Args, Debug)]
pub struct PrArgs {
    /// Path to local repository clone
    pub repo: PathBuf,
    /// Pull request number
    pub number: u64,
    /// Line delta threshold for auto Layer 2 (default: 200)
    #[arg(long, default_value = "200")]
    pub threshold: usize,
}

pub async fn run(args: PrArgs) -> Result<()> {
    let repo = &args.repo;
    let meta = github::pr_meta(repo, args.number)?;

    let base = &meta.base_ref_name;
    let head = &meta.head_ref_name;

    let result = pipeline::run(repo, base, head, args.threshold, false).await?;
    let repo_label =
        github::remote_name(repo, base, head).unwrap_or_else(|_| repo.display().to_string());
    let diff_output = render::layer1(&result, &repo_label, base, head);

    let ci_status = meta.ci_status();
    let reviews = meta.reviews.as_deref().unwrap_or(&[]);

    let output = render::pr_envelope(
        meta.number,
        &meta.title,
        &meta.body,
        &meta.state,
        ci_status,
        reviews,
        &diff_output,
    );

    print!("{output}");
    Ok(())
}
