// ABOUTME: `pasua hunk` command — Layer 3 scoped diff for a single symbol.
// ABOUTME: Returns unified diff lines scoped to the symbol's line range only.

use clap::Args;
use anyhow::Result;
use std::path::PathBuf;

use crate::core::{github, skeletal, render};

#[derive(Args, Debug)]
pub struct HunkArgs {
    /// Path to local repository clone
    pub repo: PathBuf,
    /// Base ref
    pub base: String,
    /// Head ref
    pub head: String,
    /// File path (relative to repo root)
    pub file: String,
    /// Symbol name
    pub symbol: String,
}

pub async fn run(args: HunkArgs) -> Result<()> {
    let repo = &args.repo;

    let base_bytes = github::file_at(repo, &args.base, &args.file)?.unwrap_or_default();
    let head_bytes = github::file_at(repo, &args.head, &args.file)?.unwrap_or_default();

    let base_syms = skeletal::extract(&args.file, &base_bytes)?;
    let head_syms = skeletal::extract(&args.file, &head_bytes)?;

    // Find symbol in base and head
    let base_sym = base_syms.iter().find(|s| s.name == args.symbol);
    let head_sym = head_syms.iter().find(|s| s.name == args.symbol);

    let (kind, hunk) = match (base_sym, head_sym) {
        (Some(b), Some(h)) => {
            let base_lines: Vec<&str> = std::str::from_utf8(&base_bytes)?.lines().collect();
            let head_lines: Vec<&str> = std::str::from_utf8(&head_bytes)?.lines().collect();
            let base_slice = &base_lines[b.start_line.saturating_sub(1)..b.end_line.min(base_lines.len())];
            let head_slice = &head_lines[h.start_line.saturating_sub(1)..h.end_line.min(head_lines.len())];
            let hunk = make_hunk(base_slice, head_slice);
            (b.kind, hunk)
        }
        (Some(b), None) => {
            let base_lines: Vec<&str> = std::str::from_utf8(&base_bytes)?.lines().collect();
            let base_slice = &base_lines[b.start_line.saturating_sub(1)..b.end_line.min(base_lines.len())];
            let hunk = base_slice.iter().map(|l| format!("-{l}")).collect::<Vec<_>>().join("\n");
            (b.kind, hunk)
        }
        (None, Some(h)) => {
            let head_lines: Vec<&str> = std::str::from_utf8(&head_bytes)?.lines().collect();
            let head_slice = &head_lines[h.start_line.saturating_sub(1)..h.end_line.min(head_lines.len())];
            let hunk = head_slice.iter().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n");
            (h.kind, hunk)
        }
        (None, None) => {
            anyhow::bail!("Symbol '{}' not found in {} at either ref", args.symbol, args.file);
        }
    };

    let output = render::layer3(&args.file, &args.file, &args.symbol, kind, &hunk);
    print!("{output}");
    Ok(())
}

/// Produce a minimal unified-style hunk comparing two line slices.
fn make_hunk(base: &[&str], head: &[&str]) -> String {
    let mut out = String::new();
    // Simple line-by-line diff: removed lines prefixed -, added prefixed +, common prefixed space
    let max = base.len().max(head.len());
    for i in 0..max {
        match (base.get(i), head.get(i)) {
            (Some(b), Some(h)) if b == h => out.push_str(&format!(" {b}\n")),
            (Some(b), Some(h)) => {
                out.push_str(&format!("-{b}\n"));
                out.push_str(&format!("+{h}\n"));
            }
            (Some(b), None) => out.push_str(&format!("-{b}\n")),
            (None, Some(h)) => out.push_str(&format!("+{h}\n")),
            (None, None) => {}
        }
    }
    out
}
