// ABOUTME: Gleam language support — tree-sitter grammar and gleam lsp integration.
// ABOUTME: Extracts functions, types, type aliases, and constants via tree-sitter queries.

use super::{LanguageSupport, SymbolKind};

pub struct Gleam;

impl LanguageSupport for Gleam {
    fn extensions(&self) -> &[&str] {
        &["gleam"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_gleam::LANGUAGE.into()
    }

    fn symbol_query(&self) -> &str {
        r#"
(function name: (identifier) @name) @fn

(type_definition
  (type_name name: (type_identifier) @name)) @en

(type_alias
  (type_name name: (type_identifier) @name)) @ty

(constant name: (identifier) @name) @co
"#
    }

    fn lsp_command(&self) -> &[&str] {
        &["gleam", "lsp"]
    }

    fn lsp_language_id(&self) -> &'static str {
        "gleam"
    }

    fn symbol_kind(&self, node: tree_sitter::Node<'_>, _source: &[u8]) -> Option<SymbolKind> {
        match node.kind() {
            "function" => Some(SymbolKind::Fn),
            "type_definition" => Some(SymbolKind::En),
            "type_alias" => Some(SymbolKind::Ty),
            "constant" => Some(SymbolKind::Co),
            _ => None,
        }
    }

    fn project_files(&self) -> &[&str] {
        &["gleam.toml"]
    }
}

#[cfg(test)]
mod tests {
    use crate::core::skeletal::extract;
    use crate::languages::SymbolKind;

    #[test]
    fn extracts_function() {
        let src = b"pub fn greet(name: String) -> String { \"Hello \" <> name }";
        let syms = extract("greeter.gleam", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"), "missing greet: {names:?}");
        let greet = syms.iter().find(|s| s.name == "greet").unwrap();
        assert_eq!(greet.kind, SymbolKind::Fn);
    }

    #[test]
    fn extracts_type_definition() {
        let src = b"pub type Color { Red Green Blue }";
        let syms = extract("color.gleam", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Color"), "missing Color: {names:?}");
        let color = syms.iter().find(|s| s.name == "Color").unwrap();
        assert_eq!(color.kind, SymbolKind::En);
    }

    #[test]
    fn extracts_type_alias() {
        let src = b"pub type Name = String";
        let syms = extract("alias.gleam", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Name"), "missing Name: {names:?}");
        let name = syms.iter().find(|s| s.name == "Name").unwrap();
        assert_eq!(name.kind, SymbolKind::Ty);
    }

    #[test]
    fn extracts_constant() {
        let src = b"pub const max_size: Int = 100";
        let syms = extract("consts.gleam", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"max_size"), "missing max_size: {names:?}");
        let c = syms.iter().find(|s| s.name == "max_size").unwrap();
        assert_eq!(c.kind, SymbolKind::Co);
    }
}
