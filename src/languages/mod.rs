// ABOUTME: Language support trait and symbol types.
// ABOUTME: Each language implements LanguageSupport; registry maps extension → impl.

pub mod elixir;
pub mod gleam;
pub mod go;
pub mod python;
pub mod registry;
pub mod rust;
pub mod typescript;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Language-agnostic symbol kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    /// Function or method
    Fn,
    /// Type / struct / class
    Ty,
    /// Interface / trait / protocol
    If,
    /// Enum
    En,
    /// Constant or variable declaration
    Co,
    /// Module / package
    Mo,
    /// Impl block / extension
    Im,
    /// Macro
    Ma,
}

/// A symbol extracted from a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Hash of the symbol body — used to detect modifications.
    pub body_hash: u64,
    pub start_line: usize,
    pub end_line: usize,
}

/// Language-specific support — tree-sitter grammar, symbol query, LSP command.
pub trait LanguageSupport: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn grammar(&self) -> tree_sitter::Language;
    /// Tree-sitter S-expression query. Must capture @name for symbol name and outer node.
    fn symbol_query(&self) -> &str;
    /// LSP server command, e.g. ["gopls"]
    fn lsp_command(&self) -> &[&str];
    /// LSP initializationOptions (passed in initialize request)
    fn lsp_init_options(&self) -> Value {
        serde_json::json!({})
    }
    /// Map a tree-sitter node to SymbolKind. Receives the full node and source
    /// so implementations that need content-based discrimination (e.g. Elixir)
    /// can inspect child text rather than just the node type.
    fn symbol_kind(&self, node: tree_sitter::Node<'_>, source: &[u8]) -> Option<SymbolKind>;
    /// LSP language identifier string, e.g. "go", "rust".
    fn lsp_language_id(&self) -> &'static str;
    /// Files whose presence confirms this is the right project type (any one suffices).
    fn project_files(&self) -> &[&str];
    /// Check that required project file is present. Uses project_files() by default.
    fn check_readiness(&self, path: &Path) -> Result<(), String> {
        let files = self.project_files();
        if files.iter().any(|f| path.join(f).exists()) {
            Ok(())
        } else {
            Err(format!(
                "None of {} found in {}.",
                files.join(", "),
                path.display()
            ))
        }
    }
}
