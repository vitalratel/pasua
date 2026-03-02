// ABOUTME: Go language support — tree-sitter grammar and gopls integration.
// ABOUTME: Initial language implementation; verified against real Go projects.

use super::{LanguageSupport, SymbolKind};
use std::path::Path;

pub struct Go;

impl LanguageSupport for Go {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn symbol_query(&self) -> &str {
        // Captures @name for the identifier; outer node used for body hashing.
        r#"
(function_declaration name: (identifier) @name) @fn
(method_declaration name: (field_identifier) @name) @fn
(type_declaration (type_spec name: (type_identifier) @name)) @ty
(interface_type) @if
(const_declaration) @co
(var_declaration) @co
"#
    }

    fn lsp_command(&self) -> &[&str] {
        &["gopls"]
    }

    fn symbol_kind(&self, node_kind: &str) -> Option<SymbolKind> {
        match node_kind {
            "function_declaration" | "method_declaration" => Some(SymbolKind::Fn),
            "type_declaration" => Some(SymbolKind::Ty),
            "interface_type" => Some(SymbolKind::If),
            "const_declaration" | "var_declaration" => Some(SymbolKind::Co),
            "package_clause" => Some(SymbolKind::Mo),
            _ => None,
        }
    }

    fn check_readiness(&self, path: &Path) -> Result<(), String> {
        if path.join("go.mod").exists() {
            Ok(())
        } else {
            Err(format!(
                "No go.mod found in {}. Is this a Go module?",
                path.display()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::skeletal::extract;

    #[test]
    fn symbol_kinds() {
        let go = Go;
        assert_eq!(go.symbol_kind("function_declaration"), Some(SymbolKind::Fn));
        assert_eq!(go.symbol_kind("type_declaration"), Some(SymbolKind::Ty));
        assert_eq!(go.symbol_kind("unknown"), None);
    }

    #[test]
    fn extracts_functions_and_types() {
        let src = b"
package main

type Config struct { Port int }

func New(port int) *Config {
    return &Config{Port: port}
}
";
        let syms = extract("main.go", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Config"), "missing Config: {names:?}");
        assert!(names.contains(&"New"), "missing New: {names:?}");
    }
}
