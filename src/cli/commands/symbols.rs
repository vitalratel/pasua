// ABOUTME: `pasua symbols` command — Layer 2 symbol table for one file.
// ABOUTME: Shows where each symbol moved or how it changed.

use clap::Args;
use anyhow::Result;
use std::path::PathBuf;

use crate::core::{pipeline, render};

#[derive(Args, Debug)]
pub struct SymbolsArgs {
    /// Path to local repository clone
    pub repo: PathBuf,
    /// Base ref
    pub base: String,
    /// Head ref
    pub head: String,
    /// File path (relative to repo root)
    pub file: String,
}

pub async fn run(args: SymbolsArgs) -> Result<()> {
    let diffed = pipeline::compute_symbols(&args.repo, &args.base, &args.head, &args.file)?;
    let output = render::layer2(&args.file, &diffed);
    print!("{output}");
    Ok(())
}
