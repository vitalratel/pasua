// ABOUTME: Rust language support — tree-sitter grammar and rust-analyzer integration.
// ABOUTME: Extracts functions, types, enums, traits, impls, consts, and macros via tree-sitter queries.

use super::{LanguageSupport, SymbolKind};

pub struct Rust;

impl LanguageSupport for Rust {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn symbol_query(&self) -> &str {
        r#"
(function_item name: (identifier) @name) @fn
(struct_item name: (type_identifier) @name) @ty
(enum_item name: (type_identifier) @name) @en
(type_item name: (type_identifier) @name) @ty
(trait_item name: (type_identifier) @name) @if
(impl_item type: (type_identifier) @name) @im
(impl_item type: (generic_type type: (type_identifier) @name)) @im
(mod_item name: (identifier) @name) @mo
(const_item name: (identifier) @name) @co
(static_item name: (identifier) @name) @co
(macro_definition name: (identifier) @name) @ma
"#
    }

    fn lsp_command(&self) -> &[&str] {
        &["rust-analyzer"]
    }

    fn lsp_language_id(&self) -> &'static str {
        "rust"
    }

    fn symbol_kind(&self, node: tree_sitter::Node<'_>, _source: &[u8]) -> Option<SymbolKind> {
        match node.kind() {
            "function_item" => Some(SymbolKind::Fn),
            "struct_item" | "type_item" => Some(SymbolKind::Ty),
            "enum_item" => Some(SymbolKind::En),
            "trait_item" => Some(SymbolKind::If),
            "impl_item" => Some(SymbolKind::Im),
            "mod_item" => Some(SymbolKind::Mo),
            "const_item" | "static_item" => Some(SymbolKind::Co),
            "macro_definition" => Some(SymbolKind::Ma),
            _ => None,
        }
    }

    fn project_files(&self) -> &[&str] {
        &["Cargo.toml"]
    }
}

#[cfg(test)]
mod tests {
    use crate::core::skeletal::extract;
    use crate::languages::SymbolKind;

    #[test]
    fn extracts_fn_struct_enum_trait_impl() {
        let src = b"
struct Config { port: u16 }

enum Mode { Fast, Slow }

trait Runner {
    fn run(&self);
}

impl Config {
    fn new(port: u16) -> Self {
        Config { port }
    }
}

fn main() {}

macro_rules! my_macro {
    () => {};
}
";
        let syms = extract("main.rs", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Config"), "missing Config: {names:?}");
        assert!(names.contains(&"Mode"), "missing Mode: {names:?}");
        assert!(names.contains(&"Runner"), "missing Runner: {names:?}");
        assert!(names.contains(&"new"), "missing new: {names:?}");
        assert!(names.contains(&"main"), "missing main: {names:?}");
        assert!(names.contains(&"my_macro"), "missing my_macro: {names:?}");
        let sym = |name: &str| syms.iter().find(|s| s.name == name).unwrap();
        assert_eq!(sym("Config").kind, SymbolKind::Ty);
        assert_eq!(sym("Mode").kind, SymbolKind::En);
        assert_eq!(sym("Runner").kind, SymbolKind::If);
        assert_eq!(sym("main").kind, SymbolKind::Fn);
        assert_eq!(sym("my_macro").kind, SymbolKind::Ma);
    }

    #[test]
    fn extracts_const_and_static() {
        let src = b"
const MAX: usize = 100;
static GREETING: &str = \"hello\";
";
        let syms = extract("lib.rs", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MAX"), "missing MAX: {names:?}");
        assert!(names.contains(&"GREETING"), "missing GREETING: {names:?}");
        let sym = |name: &str| syms.iter().find(|s| s.name == name).unwrap();
        assert_eq!(sym("MAX").kind, SymbolKind::Co);
        assert_eq!(sym("GREETING").kind, SymbolKind::Co);
    }

    #[test]
    fn extracts_generic_impl() {
        let src = b"
struct Wrapper<T>(T);
impl<T> Wrapper<T> {
    fn get(&self) -> &T { &self.0 }
}
";
        let syms = extract("lib.rs", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Wrapper"), "missing Wrapper: {names:?}");
        assert!(
            syms.iter()
                .any(|s| s.name == "Wrapper" && s.kind == SymbolKind::Im),
            "expected Wrapper with Im kind: {syms:?}"
        );
    }

    #[test]
    fn extracts_type_alias() {
        let src = b"type Meters = f64;";
        let syms = extract("units.rs", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Meters"), "missing Meters: {names:?}");
        assert_eq!(
            syms.iter().find(|s| s.name == "Meters").unwrap().kind,
            SymbolKind::Ty
        );
    }

    #[test]
    fn extracts_mod() {
        let src = b"
mod utils {
    pub fn helper() {}
}
";
        let syms = extract("lib.rs", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"utils"), "missing utils: {names:?}");
        assert_eq!(
            syms.iter().find(|s| s.name == "utils").unwrap().kind,
            SymbolKind::Mo
        );
    }
}
