# pasua

Semantic code diff for AI agents. Compares branches, commits, and PRs with structural understanding — not raw line noise.

## Problem

Raw git diffs of large files overflow AI context windows and burn token budgets. A file refactored into smaller modules looks *worse* in a raw diff than the original. `pasua` gives agents what they actually need: what changed semantically, at the granularity they request.

## Features

- **Layered output** — Layer 1 overview (~200 tokens), Layer 2 per-file symbols (~300 tokens), Layer 3 per-symbol hunk (~100 tokens). Agent fetches only what it needs.
- **Split detection** — detects when one file is refactored into many; annotates source and targets
- **LSP confirmation** — tree-sitter heuristics (`?`) upgraded to LSP-confirmed (`!`) when a language server is available
- **PR envelope** — wraps diff output with PR title, CI status, and review state
- **MCP server** — integrates with Claude and other MCP clients via `pasua serve`
- **CLI** — all MCP operations available from the terminal

## Installation

Build from source (requires Rust toolchain):

```bash
cargo install --git https://github.com/vitalratel/pasua
```

Language servers are optional but improve accuracy:

| Language | Server | Install |
|----------|--------|---------|
| Go | `gopls` | `go install golang.org/x/tools/gopls@latest` |
| Rust | `rust-analyzer` | `rustup component add rust-analyzer` |
| Python | `pylsp` | `uv tool install python-lsp-server` |
| TypeScript / TSX | `typescript-language-server` | `pnpm add -g typescript-language-server typescript` |
| Elixir | `elixir-ls` | See [elixir-ls releases](https://github.com/elixir-lsp/elixir-ls/releases) |
| Gleam | built-in | `gleam lsp` (ships with the Gleam toolchain) |

## CLI Usage

```bash
# File-level overview; auto-expands large and split files
pasua diff <repo> <base> <head>

# File-level overview only; no symbol expansion
pasua diff <repo> <base> <head> --depth=none

# Force symbol listing for all files
pasua diff <repo> <base> <head> --depth=symbols

# Override auto-expand threshold (default: 200 lines)
pasua diff <repo> <base> <head> --threshold=100

# Changed symbols in one file
pasua symbols <repo> <base> <head> <file>

# Exact diff for one symbol
pasua hunk <repo> <base> <head> <file> <symbol>

# PR metadata (title, CI status, reviews) with file-level diff
pasua pr <repo> <pr-number>

# File-level overview for each commit in a range
pasua log <repo> <base>..<head>

# Start MCP server
pasua serve
```

### Example output

```
owner/repo  main→feature  +420/-1840  8f

M  main.go                    +12/-8
S  tools/registry.go          +0/-850   →[tools/local.go tools/remote.go tools/mcp.go]  !
A  tools/local.go             +310/-0   ←tools/registry.go  !
A  tools/remote.go            +280/-0   ←tools/registry.go  !
A  tools/mcp.go               +95/-0    ←tools/registry.go  !
V  cmd/server.go→cmd/main.go  +0/-0
```

`!` = LSP confirmed · `?` = heuristic only (no `!` means heuristic)

## MCP Server

Add to `.mcp.json` in your project:

```json
{
  "mcpServers": {
    "pasua": {
      "command": "pasua",
      "args": ["serve"]
    }
  }
}
```

The server exposes a single `pasua` tool with operations:

| Action | Description |
|--------|-------------|
| `summary` | File-level overview only; no symbol expansion |
| `diff` | File-level overview; auto-expands large and split files |
| `symbols` | Changed symbols for a single file |
| `hunk` | Exact diff for a single symbol |
| `pr` | PR metadata (title, CI status, reviews) with file-level diff |
| `log` | File-level overview for each commit in a range |

### Typical agent workflow

```
1. pasua pr <repo> <number>
   → title, CI status, Layer 1 (~200 tokens)

2. pasua symbols <repo> <base> <head> <file>   [per file of interest]
   → where did each symbol go (~300 tokens)

3. pasua hunk <repo> <base> <head> <file> <symbol>   [per symbol of interest]
   → exact code change (~100 tokens)
```

For a PR touching 5–10 files: **500–1500 tokens** total vs. **5000–50000+** for a raw diff.

## Architecture

Single binary, two frontends over a shared core:

```
pasua
├── cli   — pasua <command> [args]
├── mcp   — pasua serve
└── core
    ├── git      — git CLI plumbing (diff stats, file contents, ref resolution)
    ├── github   — gh CLI (PR metadata, remote name)
    ├── skeletal — tree-sitter structural extraction
    ├── semantic — LSP client (symbol confirmation)
    ├── pipeline — analysis orchestration (classify, split detection, Layer 2)
    ├── diff     — symbol-level diff computation
    ├── render   — compact text output
    └── cache    — MessagePack result cache (rmp-serde)
```

CLI and MCP expose identical operations. Neither wraps the other.

## Development

```bash
cargo test
cargo fmt && cargo clippy
cargo build --release
```

Install the pre-commit hook (one-time):

```bash
git config core.hooksPath .githooks
```

## Contributing

This is a personal project. Issues and feature requests are welcome, but unsolicited pull requests will likely be closed. If you'd like to contribute, open an issue first to discuss.
