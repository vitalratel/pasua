// ABOUTME: LSP client for confirmed symbol identity via textDocument/documentSymbol.
// ABOUTME: Communicates with language servers over stdio using Content-Length framing.

use anyhow::{Context, Result};
use lsp_types::{
    ClientCapabilities, DidOpenTextDocumentParams, DocumentSymbolClientCapabilities,
    DocumentSymbolParams, InitializeParams, InitializeResult, Location, Range,
    TextDocumentClientCapabilities, TextDocumentIdentifier, TextDocumentItem, Uri,
    WindowClientCapabilities, WorkspaceClientCapabilities, WorkspaceFolder,
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
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..Default::default()
                }),
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

        tracing::debug!("LSP documentSymbol raw for {}: {:?}", path.display(), raw);
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

    /// Wait for the language server to finish its initial workspace setup.
    ///
    /// Two-phase wait:
    /// 1. Wait for a "setting up" or "load" progress token to end (required).
    /// 2. Briefly wait for a "load" token to begin (handles cold module cache).
    ///    If no load begins within a short window, assumes warm cache and returns.
    ///
    /// Responds to server requests automatically and buffers early responses.
    /// Silently succeeds on timeout — caller proceeds with partial indexing.
    pub async fn wait_for_indexing(&mut self, dur: Duration) -> Result<()> {
        let result = tokio::time::timeout(dur, self.indexing_inner()).await;
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_timeout) => {
                tracing::debug!("LSP indexing wait timed out, proceeding with available results");
                Ok(())
            }
        }
    }

    async fn indexing_inner(&mut self) -> Result<()> {
        // Phase 1: wait for the first setup/load progress token to end.
        let mut setup_token: Option<Value> = None;
        loop {
            match self.read_and_dispatch().await? {
                MessageKind::ProgressBegin { token, title } => {
                    let t = title.to_lowercase();
                    tracing::debug!("LSP ProgressBegin token={token:?} title={title:?}");
                    if t.contains("load") || t.contains("setting up") {
                        setup_token = Some(token);
                    }
                }
                MessageKind::ProgressEnd { token } => {
                    tracing::debug!("LSP ProgressEnd token={token:?} setup_token={setup_token:?}");
                    if setup_token.as_ref() == Some(&token) {
                        tracing::debug!(
                            "LSP setup/load token ended; checking for follow-on loading"
                        );
                        // Phase 2: if setup (not a "load") just ended, wait briefly for
                        // "Loading Packages" to begin (cold module cache path).
                        // If it doesn't start within the window, the module cache is warm.
                        return self.wait_for_load_after_setup().await;
                    }
                }
                MessageKind::Response { id, result } => {
                    self.pending.insert(id, result);
                }
                _ => {}
            }
        }
    }

    /// After a setup token ends, wait briefly to see if "Loading Packages" begins.
    /// If not, return immediately (warm cache). If it does, wait for it to finish.
    async fn wait_for_load_after_setup(&mut self) -> Result<()> {
        // Determine if the token that just ended was already a "load" token.
        // We don't have the title here, so use a 200ms probe window.
        let probe =
            tokio::time::timeout(Duration::from_millis(200), self.wait_for_load_begin()).await;

        match probe {
            Ok(Ok(load_token)) => {
                tracing::debug!("LSP load token started; waiting for it to end");
                // Wait for this load token to end
                loop {
                    match self.read_and_dispatch().await? {
                        MessageKind::ProgressEnd { token } if token == load_token => {
                            tracing::debug!("LSP load token ended");
                            return Ok(());
                        }
                        MessageKind::Response { id, result } => {
                            self.pending.insert(id, result);
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                // No load started — warm cache, already ready
                tracing::debug!("LSP no follow-on loading; warm cache assumed");
                Ok(())
            }
        }
    }

    /// Read messages until a "Loading Packages" begin is seen. Returns its token.
    async fn wait_for_load_begin(&mut self) -> Result<Value> {
        loop {
            match self.read_and_dispatch().await? {
                MessageKind::ProgressBegin { token, title }
                    if title.to_lowercase().contains("load") =>
                {
                    tracing::debug!("LSP load begin token={token:?} title={title:?}");
                    return Ok(token);
                }
                MessageKind::Response { id, result } => {
                    self.pending.insert(id, result);
                }
                _ => {}
            }
        }
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
        if let Some(v) = self.pending.remove(&id) {
            return Ok(v);
        }
        loop {
            if let MessageKind::Response { id: msg_id, result } = self.read_and_dispatch().await? {
                if msg_id == id {
                    return Ok(result);
                }
                self.pending.insert(msg_id, result);
            }
        }
    }

    /// Read one message and classify it, responding to server requests automatically.
    async fn read_and_dispatch(&mut self) -> Result<MessageKind> {
        let msg = self.read_message().await?;
        let obj = match msg {
            Value::Object(m) => m,
            _ => return Ok(MessageKind::Other),
        };
        let kind = classify_message(&obj);
        // Respond to server-initiated requests immediately
        if let MessageKind::ServerRequest { ref id } = kind {
            let response = json!({"jsonrpc": "2.0", "id": id, "result": null});
            let _ = self.send(response).await;
        }
        Ok(kind)
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

/// Classification of an incoming JSON-RPC message.
enum MessageKind {
    /// A response to one of our requests.
    Response { id: u64, result: Value },
    /// A request from the server (e.g. `window/workDoneProgress/create`).
    ServerRequest { id: Value },
    /// A `$/progress` begin notification. `title` describes the work item.
    ProgressBegin { token: Value, title: String },
    /// A `$/progress` end notification for a token.
    ProgressEnd { token: Value },
    /// Any other notification or unrecognised message.
    Other,
}

fn classify_message(obj: &serde_json::Map<String, Value>) -> MessageKind {
    let has_method = obj.contains_key("method");
    let has_id = obj.contains_key("id");

    match (has_method, has_id) {
        // Server-initiated request: has both method and id
        (true, true) => MessageKind::ServerRequest {
            id: obj["id"].clone(),
        },
        // Notification: has method, no id
        (true, false) => {
            if obj.get("method").and_then(|m| m.as_str()) != Some("$/progress") {
                return MessageKind::Other;
            }
            let params = obj.get("params");
            let token = params
                .and_then(|p| p.get("token"))
                .cloned()
                .unwrap_or(Value::Null);
            let value = params.and_then(|p| p.get("value"));
            let kind = value
                .and_then(|v| v.get("kind"))
                .and_then(|k| k.as_str())
                .unwrap_or("");
            match kind {
                "begin" => {
                    let title = value
                        .and_then(|v| v.get("title"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    MessageKind::ProgressBegin { token, title }
                }
                "end" => MessageKind::ProgressEnd { token },
                _ => MessageKind::Other,
            }
        }
        // Response: has id, no method
        (false, true) => {
            if let Some(id) = obj.get("id").and_then(|v| v.as_u64()) {
                let result = obj
                    .get("result")
                    .cloned()
                    .or_else(|| obj.get("error").cloned())
                    .unwrap_or(Value::Null);
                MessageKind::Response { id, result }
            } else {
                MessageKind::Other
            }
        }
        (false, false) => MessageKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn progress_notification(kind: &str) -> serde_json::Map<String, Value> {
        match json!({
            "jsonrpc": "2.0",
            "method": "$/progress",
            "params": { "token": "1", "value": { "kind": kind } }
        }) {
            Value::Object(m) => m,
            _ => unreachable!(),
        }
    }

    fn server_request(method: &str) -> serde_json::Map<String, Value> {
        match json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": {} }) {
            Value::Object(m) => m,
            _ => unreachable!(),
        }
    }

    fn response_msg(id: u64, result: Value) -> serde_json::Map<String, Value> {
        match json!({ "jsonrpc": "2.0", "id": id, "result": result }) {
            Value::Object(m) => m,
            _ => unreachable!(),
        }
    }

    #[test]
    fn indexing_end_classified() {
        let msg = progress_notification("end");
        assert!(matches!(
            classify_message(&msg),
            MessageKind::ProgressEnd { .. }
        ));
    }

    #[test]
    fn indexing_begin_classified() {
        let msg = progress_notification("begin");
        assert!(matches!(
            classify_message(&msg),
            MessageKind::ProgressBegin { .. }
        ));
    }

    #[test]
    fn server_request_classified() {
        let msg = server_request("window/workDoneProgress/create");
        assert!(matches!(
            classify_message(&msg),
            MessageKind::ServerRequest { .. }
        ));
    }

    #[test]
    fn response_classified() {
        let msg = response_msg(5, json!([1, 2, 3]));
        assert!(matches!(
            classify_message(&msg),
            MessageKind::Response { id: 5, .. }
        ));
    }

    #[test]
    fn report_notification_is_other() {
        let msg = progress_notification("report");
        assert!(matches!(classify_message(&msg), MessageKind::Other));
    }

    #[test]
    fn progress_begin_title_extracted() {
        let msg = match json!({
            "jsonrpc": "2.0",
            "method": "$/progress",
            "params": { "token": "tok1", "value": { "kind": "begin", "title": "Loading Packages" } }
        }) {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        assert!(matches!(
            classify_message(&msg),
            MessageKind::ProgressBegin { title, .. } if title == "Loading Packages"
        ));
    }
}
