// ABOUTME: Scoped hunk diff for a single symbol across two refs.
// ABOUTME: Produces a unified-style diff of the symbol's line range only.

use anyhow::Result;
use std::path::Path;

use crate::core::{cache::Cache, diff::DiffedSymbol, git, render, skeletal};

/// Produce a Layer 3 hunk diff for a single symbol.
pub fn symbol_hunk(
    repo: &Path,
    base: &str,
    head: &str,
    file: &str,
    symbol: &str,
) -> Result<String> {
    let base_bytes = git::file_at(repo, base, file)?.unwrap_or_default();
    let head_bytes = git::file_at(repo, head, file)?.unwrap_or_default();

    let base_syms = skeletal::extract(file, &base_bytes)?;
    let head_syms = skeletal::extract(file, &head_bytes)?;

    let base_sym = base_syms.iter().find(|s| s.name == symbol);
    let head_sym = head_syms.iter().find(|s| s.name == symbol);

    // Use LSP-confirmed range for head if available from a prior diff run.
    let lsp_head_range = cached_lsp_range(repo, base, head, file, symbol);

    let (kind, hunk) = match (base_sym, head_sym) {
        (Some(b), Some(h)) => {
            let (h_start, h_end) = lsp_head_range.unwrap_or((h.start_line, h.end_line));
            let bs = symbol_lines(&base_bytes, b.start_line, b.end_line)?;
            let hs = symbol_lines(&head_bytes, h_start, h_end)?;
            (b.kind, make_hunk(&bs, &hs))
        }
        (Some(b), None) => {
            let bs = symbol_lines(&base_bytes, b.start_line, b.end_line)?;
            let hunk = bs
                .iter()
                .map(|l| format!("-{l}"))
                .collect::<Vec<_>>()
                .join("\n");
            (b.kind, hunk)
        }
        (None, Some(h)) => {
            let (h_start, h_end) = lsp_head_range.unwrap_or((h.start_line, h.end_line));
            let hs = symbol_lines(&head_bytes, h_start, h_end)?;
            let hunk = hs
                .iter()
                .map(|l| format!("+{l}"))
                .collect::<Vec<_>>()
                .join("\n");
            (h.kind, hunk)
        }
        (None, None) => anyhow::bail!("Symbol '{symbol}' not found in {file}"),
    };

    Ok(render::layer3(file, file, symbol, kind, &hunk))
}

/// Look up the LSP-confirmed line range for `symbol` in the diff cache.
/// Returns None on any miss (cache absent, refs unresolvable, symbol not found).
fn cached_lsp_range(
    repo: &Path,
    base: &str,
    head: &str,
    file: &str,
    symbol: &str,
) -> Option<(usize, usize)> {
    let base_sha = git::resolve_ref(repo, base).ok()?;
    let head_sha = git::resolve_ref(repo, head).ok()?;
    let cache = Cache::new(Cache::default_path());
    let syms: Vec<DiffedSymbol> = cache.get(repo, &base_sha, &head_sha, file)?;
    syms.iter().find(|s| s.name == symbol)?.lsp_range
}

fn symbol_lines(bytes: &[u8], start: usize, end: usize) -> Result<Vec<String>> {
    let lines: Vec<String> = std::str::from_utf8(bytes)?
        .lines()
        .map(|l| l.to_owned())
        .collect();
    let slice = &lines[start.saturating_sub(1)..end.min(lines.len())];
    Ok(slice.to_vec())
}

fn make_hunk(base: &[String], head: &[String]) -> String {
    let base_text = base.join("\n");
    let head_text = head.join("\n");
    let diff = similar::TextDiff::from_lines(&base_text, &head_text);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let prefix = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => " ",
        };
        out.push_str(prefix);
        out.push_str(change.value());
        if !change.value().ends_with('\n') {
            out.push('\n');
        }
    }
    out
}
