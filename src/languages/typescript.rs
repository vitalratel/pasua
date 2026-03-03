// ABOUTME: TypeScript and TSX language support — tree-sitter grammars and typescript-language-server integration.
// ABOUTME: Extracts functions, classes, interfaces, type aliases, and enums via tree-sitter queries.

use super::{LanguageSupport, SymbolKind};

const SYMBOL_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @fn

(class_declaration name: (type_identifier) @name) @ty

(interface_declaration name: (type_identifier) @name) @if

(type_alias_declaration name: (type_identifier) @name) @ty

(enum_declaration name: (identifier) @name) @en

(lexical_declaration
  (variable_declarator name: (identifier) @name value: (arrow_function))) @fn
"#;

fn ts_symbol_kind(node: tree_sitter::Node<'_>) -> Option<SymbolKind> {
    match node.kind() {
        "function_declaration" | "lexical_declaration" => Some(SymbolKind::Fn),
        "class_declaration" | "type_alias_declaration" => Some(SymbolKind::Ty),
        "interface_declaration" => Some(SymbolKind::If),
        "enum_declaration" => Some(SymbolKind::En),
        _ => None,
    }
}

pub struct TypeScript;

impl LanguageSupport for TypeScript {
    fn extensions(&self) -> &[&str] {
        &["ts"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn symbol_query(&self) -> &str {
        SYMBOL_QUERY
    }

    fn lsp_command(&self) -> &[&str] {
        &["typescript-language-server", "--stdio"]
    }

    fn lsp_language_id(&self) -> &'static str {
        "typescript"
    }

    fn symbol_kind(&self, node: tree_sitter::Node<'_>, _source: &[u8]) -> Option<SymbolKind> {
        ts_symbol_kind(node)
    }

    fn project_files(&self) -> &[&str] {
        &["tsconfig.json", "package.json"]
    }
}

pub struct Tsx;

impl LanguageSupport for Tsx {
    fn extensions(&self) -> &[&str] {
        &["tsx"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    }

    fn symbol_query(&self) -> &str {
        SYMBOL_QUERY
    }

    fn lsp_command(&self) -> &[&str] {
        &["typescript-language-server", "--stdio"]
    }

    fn lsp_language_id(&self) -> &'static str {
        "typescriptreact"
    }

    fn symbol_kind(&self, node: tree_sitter::Node<'_>, _source: &[u8]) -> Option<SymbolKind> {
        ts_symbol_kind(node)
    }

    fn project_files(&self) -> &[&str] {
        &["tsconfig.json", "package.json"]
    }
}

#[cfg(test)]
mod tests {
    use crate::core::skeletal::extract;
    use crate::languages::SymbolKind;

    #[test]
    fn extracts_function_class_interface() {
        let src = b"
function greet(name: string): string { return name; }

class Greeter { greet(): void {} }

interface Printable { print(): void; }
";
        let syms = extract("greeter.ts", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"), "missing greet: {names:?}");
        assert!(names.contains(&"Greeter"), "missing Greeter: {names:?}");
        assert!(names.contains(&"Printable"), "missing Printable: {names:?}");
        assert_eq!(
            syms.iter().find(|s| s.name == "greet").unwrap().kind,
            SymbolKind::Fn
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "Greeter").unwrap().kind,
            SymbolKind::Ty
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "Printable").unwrap().kind,
            SymbolKind::If
        );
    }

    #[test]
    fn extracts_type_alias_and_enum() {
        let src = b"
type Name = string;
enum Color { Red, Green, Blue }
";
        let syms = extract("types.ts", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Name"), "missing Name: {names:?}");
        assert!(names.contains(&"Color"), "missing Color: {names:?}");
        assert_eq!(
            syms.iter().find(|s| s.name == "Name").unwrap().kind,
            SymbolKind::Ty
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "Color").unwrap().kind,
            SymbolKind::En
        );
    }

    #[test]
    fn extracts_arrow_function() {
        let src = b"
const greet = (name: string): string => name;
const MAX = 100;
";
        let syms = extract("arrows.ts", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"), "missing greet: {names:?}");
        assert!(
            !names.contains(&"MAX"),
            "MAX should not be extracted: {names:?}"
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "greet").unwrap().kind,
            SymbolKind::Fn
        );
    }

    #[test]
    fn extracts_exported_declarations() {
        let src = b"
export function foo() {}
export class Bar {}
export const baz = () => {};
";
        let syms = extract("mod.ts", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"), "missing foo: {names:?}");
        assert!(names.contains(&"Bar"), "missing Bar: {names:?}");
        assert!(names.contains(&"baz"), "missing baz: {names:?}");
        assert_eq!(
            syms.iter().find(|s| s.name == "foo").unwrap().kind,
            SymbolKind::Fn
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "Bar").unwrap().kind,
            SymbolKind::Ty
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "baz").unwrap().kind,
            SymbolKind::Fn
        );
    }

    #[test]
    fn tsx_extracts_component() {
        let src = b"
function App(): JSX.Element { return <div />; }

const Button = (props: ButtonProps) => <button>{props.label}</button>;

interface ButtonProps { label: string; }
";
        let syms = extract("App.tsx", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"App"), "missing App: {names:?}");
        assert!(names.contains(&"Button"), "missing Button: {names:?}");
        assert!(
            names.contains(&"ButtonProps"),
            "missing ButtonProps: {names:?}"
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "App").unwrap().kind,
            SymbolKind::Fn
        );
        assert_eq!(
            syms.iter().find(|s| s.name == "Button").unwrap().kind,
            SymbolKind::Fn
        );
    }
}
