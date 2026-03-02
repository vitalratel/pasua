// ABOUTME: `pasua symbols` command — Layer 2 symbol table for one file.
// ABOUTME: Shows where each symbol moved or how it changed.

use clap::Args;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::{github, skeletal, diff as sym_diff, render};

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
    let repo = &args.repo;

    let base_bytes = github::file_at(repo, &args.base, &args.file)?.unwrap_or_default();
    let head_bytes = github::file_at(repo, &args.head, &args.file)?.unwrap_or_default();

    let base_syms = skeletal::extract(&args.file, &base_bytes)?;
    let head_syms = skeletal::extract(&args.file, &head_bytes)?;

    let base_map = HashMap::from([(args.file.clone(), base_syms)]);
    let head_map = HashMap::from([(args.file.clone(), head_syms)]);

    let diffed = sym_diff::diff_symbols(&base_map, &head_map);
    let output = render::layer2(&args.file, &diffed);
    print!("{output}");
    Ok(())
}
