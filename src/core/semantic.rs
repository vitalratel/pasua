// ABOUTME: LSP client for confirmed symbol identity via textDocument/documentSymbol.
// ABOUTME: Communicates with language servers over stdio using Content-Length framing.

use anyhow::{Context, Result};
use lsp_types::{
    ClientCapabilities, DidOpenTextDocumentParams, DocumentSymbolClientCapabilities,
    DocumentSymbolParams, InitializeParams, InitializeResult, Location, Range,
    TextDocumentClientCapabilities, TextDocumentIdentifier, TextDocumentItem, Uri,
    WorkspaceClientCapabilities, WorkspaceFolder,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::{Duration, timeout};

fn path_to_uri(path: &Path) -> Result<Uri> {
    let s = format!("file://{}", path.display());
    s.parse::<Uri>()
        .map_err(|e| anyhow::anyhow!("Invalid URI for {}: {e}", path.display()))
}

/// A symbol found by the LSP server in a document.
#[derive(Debug, Clone)]
pub struct LspSymbol {
    pub name: String,
    pub kind: lsp_types::SymbolKind,
    pub range: Range,
}

/// LSP client — spawns a language server and communicates via JSON-RPC.
pub struct LspClient {
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    /// Pending responses indexed by request ID
    pending: HashMap<u64, Value>,
}

impl LspClient {
    /// Spawn a language server and perform the initialize handshake.
    pub async fn spawn(
        command: &[&str],
        root: &Path,
        init_options: serde_json::Value,
        request_timeout: Duration,
    ) -> Result<Self> {
        let (cmd, args) = command
            .split_first()
            .context("LSP command must not be empty")?;

        let mut process = Command::new(cmd)
            .args(args)
            .current_dir(root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to spawn LSP server: {cmd}"))?;

        let stdin = process.stdin.take().context("LSP stdin unavailable")?;
        let stdout = process.stdout.take().context("LSP stdout unavailable")?;

        let mut client = Self {
            process,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            pending: HashMap::new(),
        };

        client
            .initialize(root, init_options, request_timeout)
            .await?;
        Ok(client)
    }

    async fn initialize(
        &mut self,
        root: &Path,
        init_options: serde_json::Value,
        request_timeout: Duration,
    ) -> Result<()> {
        let root_uri = path_to_uri(root)?;
        let workspace_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();

        let params = InitializeParams {
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri,
                name: workspace_name,
            }]),
            initialization_options: Some(init_options),
            capabilities: ClientCapabilities {
                workspace: Some(WorkspaceClientCapabilities {
                    workspace_folders: Some(true),
                    ..Default::default()
                }),
                text_document: Some(TextDocumentClientCapabilities {
                    document_symbol: Some(DocumentSymbolClientCapabilities {
                        hierarchical_document_symbol_support: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let _result: InitializeResult = self.request("initialize", params, request_timeout).await?;

        // Send initialized notification (no response expected)
        self.notify("initialized", json!({})).await?;

        Ok(())
    }

    /// Open a file in the language server.
    pub async fn open_file(&mut self, path: &Path, content: &str, language_id: &str) -> Result<()> {
        let uri = path_to_uri(path)?;

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version: 1,
                text: content.to_string(),
            },
        };

        self.notify("textDocument/didOpen", serde_json::to_value(params)?)
            .await
    }

    /// Get all symbols defined in a document.
    pub async fn document_symbols(
        &mut self,
        path: &Path,
        request_timeout: Duration,
    ) -> Result<Vec<LspSymbol>> {
        let uri = path_to_uri(path)?;

        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let raw: Value = self
            .request_raw("textDocument/documentSymbol", params, request_timeout)
            .await?;

        flatten_symbols(raw)
    }

    /// Graceful shutdown.
    pub async fn shutdown(&mut self, request_timeout: Duration) -> Result<()> {
        let _: Value = self
            .request_raw("shutdown", json!(null), request_timeout)
            .await
            .unwrap_or(Value::Null);
        let _ = self.notify("exit", json!({})).await;
        let _ = self.process.kill().await;
        Ok(())
    }

    // ── JSON-RPC internals ────────────────────────────────────────────────────

    async fn request<P, R>(&mut self, method: &str, params: P, dur: Duration) -> Result<R>
    where
        P: serde::Serialize,
        R: for<'de> serde::Deserialize<'de>,
    {
        let raw = self.request_raw(method, params, dur).await?;
        Ok(serde_json::from_value(raw)?)
    }

    async fn request_raw<P: serde::Serialize>(
        &mut self,
        method: &str,
        params: P,
        dur: Duration,
    ) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;

        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.send(msg).await?;

        timeout(dur, self.recv_response(id))
            .await
            .with_context(|| format!("LSP request '{method}' timed out after {dur:?}"))?
    }

    async fn notify<P: serde::Serialize>(&mut self, method: &str, params: P) -> Result<()> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.send(msg).await
    }

    async fn send(&mut self, msg: Value) -> Result<()> {
        let body = serde_json::to_string(&msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        self.stdin.write_all(header.as_bytes()).await?;
        self.stdin.write_all(body.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn recv_response(&mut self, id: u64) -> Result<Value> {
        // Check pending cache first
        if let Some(v) = self.pending.remove(&id) {
            return Ok(v);
        }

        loop {
            let msg = self.read_message().await?;

            // Only JSON objects matter
            let obj = match msg {
                Value::Object(m) => m,
                _ => continue,
            };

            // Skip notifications (no "id" field)
            let Some(msg_id) = obj.get("id") else {
                continue;
            };

            // Match numeric id
            let msg_id_u64 = msg_id.as_u64();
            let result = obj
                .get("result")
                .cloned()
                .or_else(|| obj.get("error").cloned())
                .unwrap_or(Value::Null);

            if msg_id_u64 == Some(id) {
                return Ok(result);
            }

            // Buffer mismatched responses
            if let Some(n) = msg_id_u64 {
                self.pending.insert(n, result);
            }
        }
    }

    async fn read_message(&mut self) -> Result<Value> {
        // Read headers
        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).await?;
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                break;
            }
            if let Some(val) = line.strip_prefix("Content-Length: ") {
                content_length = val.trim().parse()?;
            }
        }

        anyhow::ensure!(content_length > 0, "LSP message with zero Content-Length");

        // Read body
        let mut body = vec![0u8; content_length];
        tokio::io::AsyncReadExt::read_exact(&mut self.stdout, &mut body).await?;
        Ok(serde_json::from_slice(&body)?)
    }
}

/// Flatten a documentSymbol response (hierarchical or flat) into a list.
fn flatten_symbols(raw: Value) -> Result<Vec<LspSymbol>> {
    if raw.is_null() {
        return Ok(vec![]);
    }

    let arr = match raw {
        Value::Array(a) => a,
        _ => return Ok(vec![]),
    };

    if arr.is_empty() {
        return Ok(vec![]);
    }

    // Detect hierarchical (DocumentSymbol has "selectionRange") vs flat (SymbolInformation has "location")
    let is_hierarchical = arr
        .first()
        .and_then(|v| v.as_object())
        .map(|o| o.contains_key("selectionRange"))
        .unwrap_or(false);

    let mut result = Vec::new();
    if is_hierarchical {
        for item in &arr {
            collect_document_symbol(item, &mut result);
        }
    } else {
        for item in &arr {
            if let Ok(s) = serde_json::from_value::<SymbolInformationCompat>(item.clone()) {
                result.push(LspSymbol {
                    name: s.name,
                    kind: s.kind,
                    range: s.location.range,
                });
            }
        }
    }

    Ok(result)
}

fn collect_document_symbol(val: &Value, out: &mut Vec<LspSymbol>) {
    if let Ok(s) = serde_json::from_value::<DocumentSymbolCompat>(val.clone()) {
        out.push(LspSymbol {
            name: s.name.clone(),
            kind: s.kind,
            range: s.selection_range,
        });
        for child in &s.children {
            collect_document_symbol(child, out);
        }
    }
}

/// Subset of DocumentSymbol for deserialization.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentSymbolCompat {
    name: String,
    kind: lsp_types::SymbolKind,
    selection_range: Range,
    #[serde(default)]
    children: Vec<Value>,
}

/// Subset of SymbolInformation for deserialization.
#[derive(Deserialize)]
struct SymbolInformationCompat {
    name: String,
    kind: lsp_types::SymbolKind,
    location: Location,
}

/// Check if an LSP server command is available on PATH.
pub fn is_available(command: &str) -> bool {
    which::which(command).is_ok()
}
