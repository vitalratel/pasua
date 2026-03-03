# Changelog

## 0.1.0

Initial release.

### CLI

- `diff` — file-level overview with automatic symbol expansion for split files and large deltas
- `diff --depth=none` — file-level-only overview; suppresses all symbol expansion
- `diff --depth=symbols` — force symbol listing for all files
- `symbols` — changed symbols for a single file
- `hunk` — scoped diff for a single symbol
- `pr` — PR envelope with title, CI status, review state, and file-level diff
- `log` — per-commit file-level overview for a commit range; outputs full commit SHAs
- `serve` — start MCP server (stdio)
- Symbol lines show line count for symbols ≥ 10 lines
- Sigil legend in `--help` output

### Configuration

- Global config file at `~/.config/pasua/config.toml` with `[defaults]`, `[lsp]`, and `[lsp.timeouts]` sections
- Environment variable overrides: `PASUA_THRESHOLD`, `PASUA_LSP_TIMEOUT`, `PASUA_LSP_INDEXING_TIMEOUT`
- Per-language LSP indexing timeouts via `PASUA_LSP_{LANG}_INDEXING_TIMEOUT` and `[lsp.timeouts]` in config file

### Core

- Split detection: heuristic (tree-sitter symbol overlap) and LSP-confirmed
- LSP confirmation via `textDocument/documentSymbol` — upgrades `?` to `!`
- MessagePack result cache keyed by repo + base SHA + head SHA + file
- Go language support via tree-sitter + gopls
- Rust language support via tree-sitter + rust-analyzer
- Python language support via tree-sitter + pylsp
- TypeScript and TSX language support via tree-sitter + typescript-language-server
- Elixir language support via tree-sitter + elixir-ls
- Gleam language support via tree-sitter + gleam lsp

### MCP

- Single `pasua` tool with actions: `summary`, `diff`, `symbols`, `hunk`, `pr`, `log`
- Server instructions include workflow guide and sigil legend
