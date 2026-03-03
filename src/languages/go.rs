// ABOUTME: Go language support — tree-sitter grammar and gopls integration.
// ABOUTME: Extracts functions, methods, types, interfaces, and constants via tree-sitter queries.

use super::{LanguageSupport, SymbolKind};

pub struct Go;

impl LanguageSupport for Go {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn symbol_query(&self) -> &str {
        r#"
(function_declaration name: (identifier) @name) @fn
(method_declaration name: (field_identifier) @name) @fn
(type_declaration (type_spec name: (type_identifier) @name)) @ty
(const_declaration (const_spec name: (identifier) @name)) @co
(var_declaration (var_spec name: (identifier) @name)) @co
"#
    }

    fn lsp_command(&self) -> &[&str] {
        &["gopls"]
    }

    fn lsp_language_id(&self) -> &'static str {
        "go"
    }

    fn symbol_kind(&self, node: tree_sitter::Node<'_>, _source: &[u8]) -> Option<SymbolKind> {
        match node.kind() {
            "function_declaration" | "method_declaration" => Some(SymbolKind::Fn),
            "type_declaration" => Some(SymbolKind::Ty),
            "const_declaration" | "var_declaration" => Some(SymbolKind::Co),
            _ => None,
        }
    }

    fn project_files(&self) -> &[&str] {
        &["go.mod"]
    }
}

#[cfg(test)]
mod tests {
    use crate::core::skeletal::extract;
    use crate::languages::SymbolKind;

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
        assert_eq!(
            syms.iter().find(|s| s.name == "Config").unwrap().kind,
            SymbolKind::Ty
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "New").unwrap().kind,
            SymbolKind::Fn
        );
    }

    #[test]
    fn extracts_method() {
        let src = b"
package main

type Server struct{}

func (s *Server) Start() error { return nil }
";
        let syms = extract("server.go", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Start"), "missing Start: {names:?}");
        assert_eq!(
            syms.iter().find(|s| s.name == "Start").unwrap().kind,
            SymbolKind::Fn
        );
    }

    #[test]
    fn extracts_interface() {
        let src = b"
package main

type Writer interface {
    Write(p []byte) (int, error)
}
";
        let syms = extract("iface.go", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Writer"), "missing Writer: {names:?}");
        assert_eq!(
            syms.iter().find(|s| s.name == "Writer").unwrap().kind,
            SymbolKind::Ty
        );
    }

    #[test]
    fn extracts_const_and_var() {
        let src = b"
package main

const MaxRetries = 3
var DefaultTimeout = 30
";
        let syms = extract("config.go", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"MaxRetries"),
            "missing MaxRetries: {names:?}"
        );
        assert!(
            names.contains(&"DefaultTimeout"),
            "missing DefaultTimeout: {names:?}"
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "MaxRetries").unwrap().kind,
            SymbolKind::Co
        );
        assert_eq!(
            syms.iter()
                .find(|s| s.name == "DefaultTimeout")
                .unwrap()
                .kind,
            SymbolKind::Co
        );
    }

    #[test]
    fn extracts_grouped_const() {
        let src = b"
package main

const (
\tA = 1
\tB = 2
)
";
        let syms = extract("consts.go", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"A"), "missing A: {names:?}");
        assert!(names.contains(&"B"), "missing B: {names:?}");
    }
}
