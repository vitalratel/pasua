// ABOUTME: Scoped hunk diff for a single symbol across two refs.
// ABOUTME: Produces a unified-style diff of the symbol's line range only.

use anyhow::Result;
use std::path::Path;

use crate::core::{github, render, skeletal};

/// Produce a Layer 3 hunk diff for a single symbol.
pub fn symbol_hunk(repo: &Path, base: &str, head: &str, file: &str, symbol: &str) -> Result<String> {
    let base_bytes = github::file_at(repo, base, file)?.unwrap_or_default();
    let head_bytes = github::file_at(repo, head, file)?.unwrap_or_default();

    let base_syms = skeletal::extract(file, &base_bytes)?;
    let head_syms = skeletal::extract(file, &head_bytes)?;

    let base_sym = base_syms.iter().find(|s| s.name == symbol);
    let head_sym = head_syms.iter().find(|s| s.name == symbol);

    let (kind, hunk) = match (base_sym, head_sym) {
        (Some(b), Some(h)) => {
            let bs = symbol_lines(&base_bytes, b.start_line, b.end_line)?;
            let hs = symbol_lines(&head_bytes, h.start_line, h.end_line)?;
            (b.kind, make_hunk(&bs, &hs))
        }
        (Some(b), None) => {
            let bs = symbol_lines(&base_bytes, b.start_line, b.end_line)?;
            let hunk = bs.iter().map(|l| format!("-{l}")).collect::<Vec<_>>().join("\n");
            (b.kind, hunk)
        }
        (None, Some(h)) => {
            let hs = symbol_lines(&head_bytes, h.start_line, h.end_line)?;
            let hunk = hs.iter().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n");
            (h.kind, hunk)
        }
        (None, None) => anyhow::bail!("Symbol '{symbol}' not found in {file}"),
    };

    Ok(render::layer3(file, file, symbol, kind, &hunk))
}

fn symbol_lines(bytes: &[u8], start: usize, end: usize) -> Result<Vec<String>> {
    let lines: Vec<String> = std::str::from_utf8(bytes)?.lines().map(|l| l.to_owned()).collect();
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
