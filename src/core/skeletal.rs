// ABOUTME: Tree-sitter structural extraction — parses source files into symbol skeletons.
// ABOUTME: Provides heuristic symbol identity without requiring LSP.

use crate::languages::{Symbol, registry};
use anyhow::Result;
use tree_sitter::StreamingIterator;

/// Extract symbols from source bytes using tree-sitter.
///
/// Returns an empty vec for unknown file extensions.
pub fn extract(path: &str, source: &[u8]) -> Result<Vec<Symbol>> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let Some(lang) = registry::for_extension(ext) else {
        return Ok(vec![]);
    };

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang.grammar())?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter parse returned None for {path}"))?;

    let query = tree_sitter::Query::new(&lang.grammar(), lang.symbol_query())?;
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);

    let name_idx = query.capture_index_for_name("name").unwrap_or(0);

    let mut symbols = Vec::new();
    while let Some(m) = matches.next() {
        let outer = m.captures.first().map(|c| c.node);
        let name_cap = m.captures.iter().find(|c| c.index == name_idx);

        if let (Some(outer), Some(name_cap)) = (outer, name_cap) {
            let name = name_cap.node.utf8_text(source)?.to_string();
            let body: &str = outer.utf8_text(source).unwrap_or("");
            let body_hash = twox_hash::XxHash64::oneshot(0, body.as_bytes());
            let kind = lang
                .symbol_kind(outer.kind())
                .unwrap_or(crate::languages::SymbolKind::Fn);

            symbols.push(Symbol {
                name,
                kind,
                body_hash,
                start_line: outer.start_position().row + 1,
                end_line: outer.end_position().row + 1,
            });
        }
    }

    Ok(symbols)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GO_SOURCE: &[u8] = b"
package main

func Hello() string {
    return \"hello\"
}

func Add(a, b int) int {
    return a + b
}
";

    #[test]
    fn extracts_go_functions() {
        let syms = extract("main.go", GO_SOURCE).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Hello"), "missing Hello, got: {names:?}");
        assert!(names.contains(&"Add"), "missing Add, got: {names:?}");
    }

    #[test]
    fn body_hash_differs_for_different_bodies() {
        let a = b"func Foo() {}";
        let b = b"func Foo() { return 1 }";
        let syms_a = extract("a.go", a).unwrap();
        let syms_b = extract("b.go", b).unwrap();
        assert_ne!(syms_a[0].body_hash, syms_b[0].body_hash);
    }

    #[test]
    fn unknown_extension_returns_empty() {
        let syms = extract("file.xyz", b"some content").unwrap();
        assert!(syms.is_empty());
    }
}
