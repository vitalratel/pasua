// ABOUTME: LSP client for confirmed symbol identity via textDocument/definition.
// ABOUTME: Communicates with language servers over stdio transport.

// Placeholder — LSP integration to be implemented.
// The LSP client will:
//   1. Spawn the language server process (e.g. gopls)
//   2. Send initialize/initialized lifecycle messages
//   3. Open text documents via textDocument/didOpen
//   4. Query textDocument/definition to confirm symbol locations
//   5. Report indexing progress to stderr
//   6. Time out after N seconds (default: 30) and fall back to heuristic output

pub struct LspClient;

impl LspClient {
    pub fn new() -> Self {
        Self
    }
}
