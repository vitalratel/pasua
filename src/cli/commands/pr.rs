// ABOUTME: `pasua pr` command — PR envelope with metadata + Layer 1 diff.
// ABOUTME: Fetches PR title, description, CI status via gh CLI.

use clap::Args;
use anyhow::Result;
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
    let repo_label = github::remote_name(repo).unwrap_or_else(|_| repo.display().to_string());
    let diff_output = render::layer1(&result, &repo_label, base, head);

    let ci_status = meta.status_check_rollup.as_deref().and_then(|checks| {
        if checks.iter().any(|c| c.conclusion.as_deref() == Some("FAILURE")) {
            Some("fail")
        } else if checks.iter().all(|c| c.conclusion.as_deref() == Some("SUCCESS")) {
            Some("pass")
        } else {
            None
        }
    });

    let review_count = meta.reviews.as_deref().map(|r| r.len()).unwrap_or(0);

    let output = render::pr_envelope(
        meta.number,
        &meta.title,
        &meta.body,
        ci_status,
        review_count,
        &diff_output,
    );

    print!("{output}");
    Ok(())
}
