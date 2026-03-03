// ABOUTME: Python language support — tree-sitter grammar and pylsp/pyright integration.
// ABOUTME: Extracts functions, classes, and decorated definitions via tree-sitter queries.

use super::{LanguageSupport, SymbolKind};

pub struct Python;

impl LanguageSupport for Python {
    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn symbol_query(&self) -> &str {
        r#"
(function_definition name: (identifier) @name) @fn

(class_definition name: (identifier) @name) @ty

(decorated_definition
  definition: (function_definition name: (identifier) @name)) @fn

(decorated_definition
  definition: (class_definition name: (identifier) @name)) @ty
"#
    }

    fn lsp_command(&self) -> &[&str] {
        &["pylsp"]
    }

    fn lsp_language_id(&self) -> &'static str {
        "python"
    }

    fn symbol_kind(&self, node: tree_sitter::Node<'_>, _source: &[u8]) -> Option<SymbolKind> {
        match node.kind() {
            "function_definition" => Some(SymbolKind::Fn),
            "class_definition" => Some(SymbolKind::Ty),
            "decorated_definition" => match node.child_by_field_name("definition")?.kind() {
                "function_definition" => Some(SymbolKind::Fn),
                "class_definition" => Some(SymbolKind::Ty),
                _ => None,
            },
            _ => None,
        }
    }

    fn project_files(&self) -> &[&str] {
        &["pyproject.toml", "setup.py", "setup.cfg"]
    }
}

#[cfg(test)]
mod tests {
    use crate::core::skeletal::extract;
    use crate::languages::SymbolKind;

    #[test]
    fn extracts_function_and_class() {
        let src = b"
def greet(name):
    return 'hello ' + name

class Greeter:
    def greet(self):
        pass
";
        let syms = extract("greeter.py", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"), "missing greet: {names:?}");
        assert!(names.contains(&"Greeter"), "missing Greeter: {names:?}");
        let greet = syms
            .iter()
            .find(|s| s.name == "greet" && s.kind == SymbolKind::Fn);
        assert!(greet.is_some(), "greet should be Fn");
        let greeter = syms.iter().find(|s| s.name == "Greeter");
        assert_eq!(greeter.unwrap().kind, SymbolKind::Ty);
    }

    #[test]
    fn extracts_decorated_function_and_class() {
        let src = b"
@staticmethod
def greet():
    pass

@dataclass
class Point:
    x: int
";
        let syms = extract("stuff.py", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"), "missing greet: {names:?}");
        assert!(names.contains(&"Point"), "missing Point: {names:?}");
        let greet = syms.iter().find(|s| s.name == "greet").unwrap();
        assert_eq!(greet.kind, SymbolKind::Fn);
        let point = syms.iter().find(|s| s.name == "Point").unwrap();
        assert_eq!(point.kind, SymbolKind::Ty);
    }
}
