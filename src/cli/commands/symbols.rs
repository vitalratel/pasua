// ABOUTME: `pasua symbols` command — Layer 2 symbol table for one file.
// ABOUTME: Shows where each symbol moved or how it changed.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::core::{config, pipeline, render};

#[derive(Args, Debug)]
pub struct SymbolsArgs {
    /// Path to local repository clone
    pub repo: PathBuf,
    /// Base ref (branch, commit, or tag)
    pub base: String,
    /// Head ref (branch, commit, or tag)
    pub head: String,
    /// File path (relative to repo root)
    pub file: String,
}

pub async fn run(args: SymbolsArgs) -> Result<()> {
    let cfg = config::Config::load();
    let diffed =
        pipeline::symbols_confirmed(&args.repo, &args.base, &args.head, &args.file, &cfg).await?;
    let output = render::layer2(&args.file, &diffed);
    print!("{output}");
    Ok(())
}
