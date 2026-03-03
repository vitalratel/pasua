// ABOUTME: Elixir language support — tree-sitter grammar and elixir-ls integration.
// ABOUTME: Extracts modules, functions, macros, protocols, and implementations via tree-sitter queries.

use super::{LanguageSupport, SymbolKind};

pub struct Elixir;

impl LanguageSupport for Elixir {
    fn extensions(&self) -> &[&str] {
        &["ex", "exs"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_elixir::LANGUAGE.into()
    }

    fn symbol_query(&self) -> &str {
        // Elixir represents all definitions as `call` nodes. Two structural patterns:
        //   def/defp/defmacro/defmacrop — first argument is the function head (call)
        //   defmodule/defprotocol/defimpl — first argument is the module name (alias)
        // symbol_kind() inspects the call target text to classify and filter non-definition calls.
        r#"
(call
  (arguments
    (call target: (identifier) @name))
  (do_block)) @call

(call
  (arguments
    (alias) @name)
  (do_block)) @call
"#
    }

    fn lsp_command(&self) -> &[&str] {
        &["elixir-ls"]
    }

    fn lsp_language_id(&self) -> &'static str {
        "elixir"
    }

    fn symbol_kind(&self, node: tree_sitter::Node<'_>, source: &[u8]) -> Option<SymbolKind> {
        let target = node.child_by_field_name("target")?;
        let text = target.utf8_text(source).ok()?;
        match text {
            "def" | "defp" => Some(SymbolKind::Fn),
            "defmodule" => Some(SymbolKind::Mo),
            "defmacro" | "defmacrop" => Some(SymbolKind::Ma),
            "defprotocol" => Some(SymbolKind::If),
            "defimpl" => Some(SymbolKind::Im),
            _ => None,
        }
    }

    fn project_files(&self) -> &[&str] {
        &["mix.exs"]
    }
}

#[cfg(test)]
mod tests {
    use crate::core::skeletal::extract;
    use crate::languages::SymbolKind;

    #[test]
    fn extracts_def_and_defp() {
        let src = b"
defmodule MyApp.Greeter do
  def greet(name) do
    \"Hello #{name}\"
  end

  defp format(s) do
    String.upcase(s)
  end
end
";
        let syms = extract("greeter.ex", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"MyApp.Greeter"),
            "missing module: {names:?}"
        );
        assert!(names.contains(&"greet"), "missing greet: {names:?}");
        assert!(names.contains(&"format"), "missing format: {names:?}");
        let sym = |name: &str| syms.iter().find(|s| s.name == name).unwrap();
        assert_eq!(sym("MyApp.Greeter").kind, SymbolKind::Mo);
        assert_eq!(sym("greet").kind, SymbolKind::Fn);
        assert_eq!(sym("format").kind, SymbolKind::Fn);
    }

    #[test]
    fn extracts_defprotocol_and_defimpl() {
        let src = b"
defprotocol Stringify do
  def to_string(t)
end

defimpl Stringify, for: Integer do
  def to_string(i), do: Integer.to_string(i)
end
";
        let syms = extract("stringify.ex", src).unwrap();
        assert!(
            syms.iter()
                .any(|s| s.name == "Stringify" && s.kind == SymbolKind::If),
            "expected Stringify as If (protocol): {syms:?}"
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "Stringify" && s.kind == SymbolKind::Im),
            "expected Stringify as Im (impl): {syms:?}"
        );
    }

    #[test]
    fn extracts_defmacro() {
        let src = b"
defmodule MyMacros do
  defmacro unless(condition, do: block) do
    quote do
      if !unquote(condition), do: unquote(block)
    end
  end
end
";
        let syms = extract("macros.ex", src).unwrap();
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"unless"), "missing macro: {names:?}");
        let unless = syms.iter().find(|s| s.name == "unless").unwrap();
        assert_eq!(unless.kind, SymbolKind::Ma);
    }

    #[test]
    fn ignores_non_definition_calls() {
        let src = b"
IO.puts(\"hello\")
Enum.map([1, 2, 3], fn x -> x * 2 end)
";
        let syms = extract("script.exs", src).unwrap();
        assert!(syms.is_empty(), "expected no symbols, got: {syms:?}");
    }
}
