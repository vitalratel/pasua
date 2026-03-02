// ABOUTME: Language support trait and symbol types.
// ABOUTME: Each language implements LanguageSupport; registry maps extension → impl.

pub mod go;
pub mod registry;

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
    /// Map tree-sitter node kind string to SymbolKind.
    fn symbol_kind(&self, node_kind: &str) -> Option<SymbolKind>;
    /// Check that required tooling is present (e.g. go.mod exists).
    fn check_readiness(&self, path: &Path) -> Result<(), String>;
}
