// ABOUTME: Compact text rendering of diff output layers.
// ABOUTME: Produces the token-efficient notation defined in the spec.

use crate::core::diff::{DiffedSymbol, SymbolStatus};
use crate::languages::SymbolKind;

/// Confidence sigil.
fn confidence(confirmed: bool) -> &'static str {
    if confirmed { "!" } else { "?" }
}

/// Render a symbol kind as its 2-char sigil.
pub fn kind_sigil(kind: SymbolKind) -> &'static str {
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

/// Render Layer 2 output for a file's symbols.
pub fn render_symbols(file: &str, symbols: &[DiffedSymbol]) -> String {
    let mut out = format!("{file}:\n");
    for sym in symbols {
        let status = render_symbol_status(&sym.status);
        let conf = confidence(sym.confirmed);
        out.push_str(&format!("  fn  {:<20} {status:<30} {conf}\n", sym.name));
    }
    out
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
        SymbolStatus::Unchanged => "".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diff::DiffedSymbol;

    fn sym(name: &str, status: SymbolStatus, confirmed: bool) -> DiffedSymbol {
        DiffedSymbol {
            name: name.to_string(),
            file: "test.go".to_string(),
            status,
            confirmed,
        }
    }

    #[test]
    fn render_added_unconfirmed() {
        let syms = vec![sym("Foo", SymbolStatus::Added, false)];
        let out = render_symbols("pkg/foo.go", &syms);
        assert!(out.contains("pkg/foo.go:"));
        assert!(out.contains('+'));
        assert!(out.contains('?'));
    }

    #[test]
    fn render_moved_confirmed() {
        let syms = vec![sym(
            "Bar",
            SymbolStatus::Moved {
                to_file: "pkg/bar.go".to_string(),
            },
            true,
        )];
        let out = render_symbols("pkg/old.go", &syms);
        assert!(out.contains("→ pkg/bar.go"));
        assert!(out.contains('!'));
    }
}
