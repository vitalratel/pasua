// ABOUTME: Compact text rendering for all output layers.
// ABOUTME: Produces the token-efficient notation defined in the spec.

use crate::core::diff::{DiffedSymbol, SymbolStatus};
use crate::core::pipeline::{DiffResult, FileDiff, FileStatus};
use crate::languages::SymbolKind;

/// Render Layer 1 overview for a diff.
///
/// `repo_label`: e.g. "owner/repo"  `base`: base ref  `head`: head ref
pub fn layer1(result: &DiffResult, repo_label: &str, base: &str, head: &str) -> String {
    let mut out = String::new();

    // Header line: repo  base→head  +total/-total  Nf
    out.push_str(&format!(
        "{repo_label}  {base}→{head}  +{}/−{}  {}f\n",
        result.summary.total_added, result.summary.total_removed, result.summary.file_count
    ));
    out.push('\n');

    for file in &result.files {
        out.push_str(&render_file_line(file));
        out.push('\n');

        // If Layer 2 symbols are present, render them indented
        if let Some(syms) = &file.symbols {
            for sym in syms {
                if sym.status != SymbolStatus::Unchanged {
                    out.push_str(&render_symbol_line(sym));
                    out.push('\n');
                }
            }
        }
    }

    out
}

/// Public version for the log command.
pub fn file_line_only(file: &FileDiff) -> String {
    render_file_line(file)
}

fn render_file_line(file: &FileDiff) -> String {
    let (sigil, path_display, annotation) = match &file.status {
        FileStatus::Modified => ("M", file.path.clone(), String::new()),
        FileStatus::Added { source } => {
            let ann = match source {
                Some(s) => format!("  ←{s}"),
                None => String::new(),
            };
            ("A", file.path.clone(), ann)
        }
        FileStatus::Deleted { targets } => {
            let ann = if targets.is_empty() {
                String::new()
            } else {
                format!("  →[{}]", targets.join(" "))
            };
            ("D", file.path.clone(), ann)
        }
        FileStatus::Split { targets } => {
            let ann = format!("  →[{}]", targets.join(" "));
            ("S", file.path.clone(), ann)
        }
        FileStatus::Renamed { old_path, new_path } => {
            ("V", format!("{old_path}→{new_path}"), String::new())
        }
    };

    let conf = if file.confirmed { "  !" } else { "" };
    let delta = if file.added > 0 || file.removed > 0 {
        format!("+{}/-{}", file.added, file.removed)
    } else {
        "+0/-0".to_string()
    };

    format!("{sigil}  {path_display:<35} {delta:<12}{annotation}{conf}")
}

fn render_symbol_line(sym: &DiffedSymbol) -> String {
    let status = render_symbol_status(&sym.status);
    let conf = if sym.confirmed { "!" } else { "?" };
    format!(
        "  {:<2}  {:<24} {:<28} {conf}",
        kind_sigil(sym.kind),
        sym.name,
        status
    )
}

fn render_symbol_status(status: &SymbolStatus) -> String {
    match status {
        SymbolStatus::Added => "+".to_string(),
        SymbolStatus::Removed => "-".to_string(),
        SymbolStatus::Modified => "*".to_string(),
        SymbolStatus::Moved { to_file } => format!("→ {to_file}"),
        SymbolStatus::MovedModified { to_file } => format!("*→ {to_file}"),
        SymbolStatus::Renamed { new_name } => format!("~ {new_name}"),
        SymbolStatus::RenamedModified { new_name } => format!("*~ {new_name}"),
        SymbolStatus::MovedRenamedModified { to_file, new_name } => {
            format!("*→ {to_file} ~ {new_name}")
        }
        SymbolStatus::Unchanged => String::new(),
    }
}

/// Render Layer 2 symbol table for a single file.
pub fn layer2(file_path: &str, symbols: &[DiffedSymbol]) -> String {
    let mut out = format!("{file_path}:\n");
    for sym in symbols {
        if sym.status != SymbolStatus::Unchanged {
            out.push_str(&render_symbol_line(sym));
            out.push('\n');
        }
    }
    out
}

/// Render Layer 3 hunk diff for a single symbol.
pub fn layer3(
    old_file: &str,
    new_file: &str,
    symbol: &str,
    kind: SymbolKind,
    hunk: &str,
) -> String {
    let kind_str = kind_sigil(kind);
    format!(
        "--- {old_file}  {kind_str} {symbol}\n+++ {new_file}  {kind_str} {symbol}\n{hunk}"
    )
}

/// PR envelope around a Layer 1 diff.
pub fn pr_envelope(
    number: u64,
    title: &str,
    body: &str,
    ci_status: Option<&str>,
    review_count: usize,
    diff_output: &str,
) -> String {
    let ci = ci_status.map(|s| format!(" [ci:{s}]")).unwrap_or_default();
    let reviews = if review_count > 0 {
        format!(" [reviews:{review_count}]")
    } else {
        String::new()
    };

    let mut out = format!("PR#{number} \"{title}\"{ci}{reviews}\n");
    if !body.is_empty() {
        // First non-empty paragraph only
        let first_para = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        if !first_para.is_empty() {
            out.push_str(&format!("> {first_para}\n"));
        }
    }
    out.push_str("---\n");
    out.push_str(diff_output);
    out
}

fn kind_sigil(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Fn => "fn",
        SymbolKind::Ty => "ty",
        SymbolKind::If => "if",
        SymbolKind::En => "en",
        SymbolKind::Co => "co",
        SymbolKind::Mo => "mo",
        SymbolKind::Im => "im",
        SymbolKind::Ma => "ma",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diff::DiffedSymbol;
    use crate::core::pipeline::DiffSummary;
    use crate::languages::SymbolKind;

    fn make_result(files: Vec<FileDiff>) -> DiffResult {
        let total_added = files.iter().map(|f| f.added).sum();
        let total_removed = files.iter().map(|f| f.removed).sum();
        let file_count = files.len();
        DiffResult {
            summary: DiffSummary {
                total_added,
                total_removed,
                file_count,
            },
            files,
        }
    }

    fn plain_file(path: &str, status: FileStatus, added: usize, removed: usize) -> FileDiff {
        FileDiff {
            status,
            path: path.to_string(),
            added,
            removed,
            symbols: None,
            confirmed: false,
        }
    }

    #[test]
    fn layer1_header_format() {
        let result = make_result(vec![plain_file("main.go", FileStatus::Modified, 12, 8)]);
        let out = layer1(&result, "owner/repo", "main", "feature");
        assert!(out.starts_with("owner/repo  main→feature  +12/−8  1f"));
    }

    #[test]
    fn layer1_modified_file() {
        let result = make_result(vec![plain_file("main.go", FileStatus::Modified, 12, 8)]);
        let out = layer1(&result, "r", "a", "b");
        assert!(out.contains("M  main.go"));
        assert!(out.contains("+12/-8"));
    }

    #[test]
    fn layer1_split_file() {
        let result = make_result(vec![plain_file(
            "registry.go",
            FileStatus::Split {
                targets: vec!["local.go".into(), "remote.go".into()],
            },
            0,
            850,
        )]);
        let out = layer1(&result, "r", "a", "b");
        assert!(out.contains("S  registry.go"));
        assert!(out.contains("→[local.go remote.go]"));
    }

    #[test]
    fn layer1_renamed_file() {
        let result = make_result(vec![plain_file(
            "new.go",
            FileStatus::Renamed {
                old_path: "cmd/server.go".into(),
                new_path: "cmd/main.go".into(),
            },
            0,
            0,
        )]);
        let out = layer1(&result, "r", "a", "b");
        assert!(out.contains("V  cmd/server.go→cmd/main.go"));
    }

    #[test]
    fn layer2_shows_changed_symbols_only() {
        let syms = vec![
            DiffedSymbol {
                name: "Foo".into(),
                file: "a.go".into(),
                kind: SymbolKind::Fn,
                status: SymbolStatus::Modified,
                confirmed: false,
            },
            DiffedSymbol {
                name: "Bar".into(),
                file: "a.go".into(),
                kind: SymbolKind::Ty,
                status: SymbolStatus::Unchanged,
                confirmed: false,
            },
        ];
        let out = layer2("a.go", &syms);
        assert!(out.contains("Foo"));
        assert!(!out.contains("Bar"));
    }

    #[test]
    fn pr_envelope_format() {
        let out = pr_envelope(42, "Fix thing", "Fixes the bug.", Some("pass"), 1, "diff here");
        assert!(out.starts_with("PR#42 \"Fix thing\" [ci:pass] [reviews:1]"));
        assert!(out.contains("> Fixes the bug."));
        assert!(out.contains("---\ndiff here"));
    }
}
