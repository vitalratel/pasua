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
            let base_lines: Vec<&str> = std::str::from_utf8(&base_bytes)?.lines().collect();
            let head_lines: Vec<&str> = std::str::from_utf8(&head_bytes)?.lines().collect();
            let bs = &base_lines[b.start_line.saturating_sub(1)..b.end_line.min(base_lines.len())];
            let hs = &head_lines[h.start_line.saturating_sub(1)..h.end_line.min(head_lines.len())];
            (b.kind, make_hunk(bs, hs))
        }
        (Some(b), None) => {
            let base_lines: Vec<&str> = std::str::from_utf8(&base_bytes)?.lines().collect();
            let bs = &base_lines[b.start_line.saturating_sub(1)..b.end_line.min(base_lines.len())];
            let hunk = bs.iter().map(|l| format!("-{l}")).collect::<Vec<_>>().join("\n");
            (b.kind, hunk)
        }
        (None, Some(h)) => {
            let head_lines: Vec<&str> = std::str::from_utf8(&head_bytes)?.lines().collect();
            let hs = &head_lines[h.start_line.saturating_sub(1)..h.end_line.min(head_lines.len())];
            let hunk = hs.iter().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n");
            (h.kind, hunk)
        }
        (None, None) => anyhow::bail!("Symbol '{symbol}' not found in {file}"),
    };

    Ok(render::layer3(file, file, symbol, kind, &hunk))
}

fn make_hunk(base: &[&str], head: &[&str]) -> String {
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
