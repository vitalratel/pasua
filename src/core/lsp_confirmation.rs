// ABOUTME: Reconciles LSP document symbols against tree-sitter diffed symbols.
// ABOUTME: Upgrades heuristic symbol entries to LSP-confirmed with line ranges.

use crate::core::diff::DiffedSymbol;
use crate::core::semantic::LspSymbol;
use crate::languages::SymbolKind;

/// Apply LSP confirmation to a slice of diffed symbols.
///
/// For each symbol confirmed by the LSP, sets `confirmed = true` and records
/// the LSP-provided line range.
pub fn apply_lsp_confirmation(lsp_syms: &[LspSymbol], diffed: &mut [DiffedSymbol]) {
    for sym in diffed.iter_mut() {
        if let Some(ls) = lsp_syms
            .iter()
            .find(|ls| lsp_bare_name(&ls.name) == sym.name && lsp_kind_matches(ls.kind, sym.kind))
        {
            sym.confirmed = true;
            sym.lsp_range = Some((
                ls.range.start.line as usize + 1,
                ls.range.end.line as usize + 1,
            ));
        }
    }
}

/// Strip receiver/qualifier prefix from an LSP symbol name.
///
/// LSP servers (e.g. gopls) qualify method names: "(*Server).Foo" → "Foo".
/// Tree-sitter extracts only the bare name, so we normalise before matching.
pub fn lsp_bare_name(name: &str) -> &str {
    name.rfind('.').map(|i| &name[i + 1..]).unwrap_or(name)
}

/// Match an LSP symbol kind against our language-agnostic SymbolKind.
///
/// LSP SymbolKind has ~26 values; we map them to our 8 semantic categories.
/// Intentionally permissive: a match on name alone without kind disagreement is
/// better than a false negative from an overly strict kind check.
fn lsp_kind_matches(lsp: lsp_types::SymbolKind, ours: SymbolKind) -> bool {
    use lsp_types::SymbolKind as L;
    match ours {
        SymbolKind::Fn => matches!(lsp, L::FUNCTION | L::METHOD | L::CONSTRUCTOR),
        SymbolKind::Ty => matches!(lsp, L::CLASS | L::STRUCT | L::OBJECT | L::TYPE_PARAMETER),
        SymbolKind::If => matches!(lsp, L::INTERFACE),
        SymbolKind::En => matches!(lsp, L::ENUM | L::ENUM_MEMBER),
        SymbolKind::Co => matches!(lsp, L::CONSTANT | L::VARIABLE | L::FIELD | L::PROPERTY),
        SymbolKind::Mo => matches!(lsp, L::MODULE | L::NAMESPACE | L::PACKAGE),
        SymbolKind::Im => matches!(lsp, L::CLASS | L::OBJECT | L::MODULE),
        SymbolKind::Ma => matches!(lsp, L::FUNCTION | L::OPERATOR),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diff::SymbolStatus;
    use lsp_types::{Position, Range};

    fn make_diffed(name: &str, kind: SymbolKind) -> DiffedSymbol {
        DiffedSymbol {
            name: name.to_string(),
            kind,
            file: "test.go".to_string(),
            status: SymbolStatus::Modified,
            confirmed: false,
            lsp_range: None,
        }
    }

    fn make_lsp(name: &str, kind: lsp_types::SymbolKind, start: u32, end: u32) -> LspSymbol {
        LspSymbol {
            name: name.to_string(),
            kind,
            range: Range {
                start: Position {
                    line: start,
                    character: 0,
                },
                end: Position {
                    line: end,
                    character: 0,
                },
            },
        }
    }

    #[test]
    fn sets_confirmed_and_range() {
        let mut diffed = vec![make_diffed("Foo", SymbolKind::Fn)];
        let lsp_syms = vec![make_lsp("Foo", lsp_types::SymbolKind::FUNCTION, 5, 10)];
        apply_lsp_confirmation(&lsp_syms, &mut diffed);
        assert!(diffed[0].confirmed);
        assert_eq!(diffed[0].lsp_range, Some((6, 11)));
    }

    #[test]
    fn ignores_missing_symbols() {
        let mut diffed = vec![make_diffed("Bar", SymbolKind::Fn)];
        let lsp_syms = vec![make_lsp("Foo", lsp_types::SymbolKind::FUNCTION, 5, 10)];
        apply_lsp_confirmation(&lsp_syms, &mut diffed);
        assert!(!diffed[0].confirmed);
        assert_eq!(diffed[0].lsp_range, None);
    }

    #[test]
    fn matches_qualified_method_name() {
        let mut diffed = vec![make_diffed("handleFoo", SymbolKind::Fn)];
        let lsp_syms = vec![make_lsp(
            "(*Server).handleFoo",
            lsp_types::SymbolKind::METHOD,
            5,
            10,
        )];
        apply_lsp_confirmation(&lsp_syms, &mut diffed);
        assert!(diffed[0].confirmed);
    }

    #[test]
    fn rejects_kind_mismatch() {
        let mut diffed = vec![make_diffed("Foo", SymbolKind::Fn)];
        let lsp_syms = vec![make_lsp("Foo", lsp_types::SymbolKind::STRUCT, 5, 10)];
        apply_lsp_confirmation(&lsp_syms, &mut diffed);
        assert!(!diffed[0].confirmed);
        assert_eq!(diffed[0].lsp_range, None);
    }

    #[test]
    fn partial_match() {
        let mut diffed = vec![
            make_diffed("Foo", SymbolKind::Fn),
            make_diffed("Bar", SymbolKind::Fn),
        ];
        let lsp_syms = vec![make_lsp("Foo", lsp_types::SymbolKind::FUNCTION, 1, 5)];
        apply_lsp_confirmation(&lsp_syms, &mut diffed);
        assert!(diffed[0].confirmed);
        assert!(!diffed[1].confirmed);
    }

    #[test]
    fn bare_name_strips_receiver() {
        assert_eq!(lsp_bare_name("(*Server).handleFoo"), "handleFoo");
        assert_eq!(lsp_bare_name("(Server).handleFoo"), "handleFoo");
        assert_eq!(lsp_bare_name("handleFoo"), "handleFoo");
    }
}
