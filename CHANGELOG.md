# Changelog

## Unreleased

### CLI
- `diff --depth=none` — suppress all symbol expansion for a file-level-only overview
- `log` now outputs full commit SHAs (previously truncated to 7 chars)
- Symbol lines in Layer 2 output now show line count for symbols ≥ 10 lines
- `--threshold` now reads from `PASUA_THRESHOLD` env var and `~/.config/pasua/config.toml` when not explicitly provided

### MCP
- `summary` action — file-level overview with no symbol expansion
- Server instructions and tool description now include sigil legend and workflow guide

### Configuration
- Global config file at `~/.config/pasua/config.toml` with `[defaults]`, `[lsp]`, and `[lsp.timeouts]` sections
- Environment variable overrides: `PASUA_THRESHOLD`, `PASUA_LSP_TIMEOUT`, `PASUA_LSP_INDEXING_TIMEOUT`
- Per-language LSP indexing timeouts via `PASUA_LSP_{LANG}_INDEXING_TIMEOUT` and `[lsp.timeouts]` in config file

## 0.1.0

Initial release.

### CLI

- `diff` — Layer 1 overview with automatic Layer 2 for split files and large deltas
- `symbols` — Layer 2 symbol table for a single file
- `hunk` — Layer 3 scoped diff for a single symbol
- `pr` — PR envelope with title, CI status, review state, and Layer 1 diff
- `log` — per-commit mini-overview for a commit range
- `serve` — start MCP server (stdio)
- `--depth=symbols` flag to force Layer 2 for all files
- `--threshold=N` to override auto-include line delta threshold (default: 200)

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

- Single `pasua` tool with actions: `diff`, `symbols`, `hunk`, `pr`, `log`
