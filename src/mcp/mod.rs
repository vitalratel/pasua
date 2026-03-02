// ABOUTME: MCP server exposing pasua operations as tools.
// ABOUTME: Identical operations to CLI; neither wraps the other.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use rmcp::{ServerHandler, handler::server::{router::tool::ToolRouter, wrapper::Parameters}, model::*, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::core::{github, pipeline, render, skeletal, diff as sym_diff};

/// MCP server for pasua operations.
#[derive(Clone)]
pub struct PasuaServer {
    tool_router: ToolRouter<Self>,
}

/// Parameters for all pasua operations.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PasuaParams {
    /// Operation: diff | symbols | hunk | pr | log
    pub action: String,

    /// Path to local repository clone (required for all operations)
    pub repo: String,

    /// Base ref — branch, commit, or tag (diff/symbols/hunk/log)
    #[serde(default)]
    pub base: Option<String>,

    /// Head ref — branch, commit, or tag (diff/symbols/hunk/log)
    #[serde(default)]
    pub head: Option<String>,

    /// File path relative to repo root (symbols/hunk)
    #[serde(default)]
    pub file: Option<String>,

    /// Symbol name (hunk only)
    #[serde(default)]
    pub symbol: Option<String>,

    /// PR number (pr only)
    #[serde(default)]
    pub pr_number: Option<u64>,

    /// Commit range e.g. main..feature (log only)
    #[serde(default)]
    pub range: Option<String>,

    /// Line delta threshold for auto Layer 2 (default: 200)
    #[serde(default)]
    pub threshold: Option<usize>,
}

#[tool_router]
impl PasuaServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Semantic code diff for AI agents. Operations: diff, symbols, hunk, pr, log.
    #[tool(
        name = "pasua",
        description = "Semantic diff: diff <repo> <base> <head> | symbols <repo> <base> <head> <file> | hunk <repo> <base> <head> <file> <symbol> | pr <repo> <pr_number> | log <repo> <range>"
    )]
    async fn pasua(&self, params: Parameters<PasuaParams>) -> Result<String, String> {
        self.execute(params.0).await
    }
}

#[tool_handler]
impl ServerHandler for PasuaServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "pasua".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: None,
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: None,
        }
    }
}

impl PasuaServer {
    async fn execute(&self, params: PasuaParams) -> Result<String, String> {
        let repo = PathBuf::from(&params.repo);
        let threshold = params.threshold.unwrap_or(200);

        match params.action.as_str() {
            "diff" => {
                let base = require(&params.base, "base")?;
                let head = require(&params.head, "head")?;
                let result = pipeline::run(&repo, base, head, threshold, false).await.map_err(|e| e.to_string())?;
                let repo_label = github::remote_name(&repo).unwrap_or_else(|_| params.repo.clone());
                Ok(render::layer1(&result, &repo_label, base, head))
            }
            "symbols" => {
                let base = require(&params.base, "base")?;
                let head = require(&params.head, "head")?;
                let file = require(&params.file, "file")?;
                let base_bytes = github::file_at(&repo, base, file).map_err(|e| e.to_string())?.unwrap_or_default();
                let head_bytes = github::file_at(&repo, head, file).map_err(|e| e.to_string())?.unwrap_or_default();
                let base_syms = skeletal::extract(file, &base_bytes).map_err(|e| e.to_string())?;
                let head_syms = skeletal::extract(file, &head_bytes).map_err(|e| e.to_string())?;
                let base_map = HashMap::from([(file.to_string(), base_syms)]);
                let head_map = HashMap::from([(file.to_string(), head_syms)]);
                let diffed = sym_diff::diff_symbols(&base_map, &head_map);
                Ok(render::layer2(file, &diffed))
            }
            "hunk" => {
                let base = require(&params.base, "base")?;
                let head = require(&params.head, "head")?;
                let file = require(&params.file, "file")?;
                let symbol = require(&params.symbol, "symbol")?;
                let base_bytes = github::file_at(&repo, base, file).map_err(|e| e.to_string())?.unwrap_or_default();
                let head_bytes = github::file_at(&repo, head, file).map_err(|e| e.to_string())?.unwrap_or_default();
                let base_syms = skeletal::extract(file, &base_bytes).map_err(|e| e.to_string())?;
                let head_syms = skeletal::extract(file, &head_bytes).map_err(|e| e.to_string())?;
                let base_sym = base_syms.iter().find(|s| s.name == symbol);
                let head_sym = head_syms.iter().find(|s| s.name == symbol);
                match (base_sym, head_sym) {
                    (None, None) => Err(format!("Symbol '{symbol}' not found in {file}")),
                    (b, h) => {
                        let bl: Vec<&str> = std::str::from_utf8(&base_bytes).unwrap_or("").lines().collect();
                        let hl: Vec<&str> = std::str::from_utf8(&head_bytes).unwrap_or("").lines().collect();
                        let bs = b.map(|s| &bl[s.start_line.saturating_sub(1)..s.end_line.min(bl.len())]).unwrap_or(&[]);
                        let hs = h.map(|s| &hl[s.start_line.saturating_sub(1)..s.end_line.min(hl.len())]).unwrap_or(&[]);
                        let kind = b.or(h).unwrap().kind;
                        let hunk = crate::cli::commands::hunk::make_hunk(bs, hs);
                        Ok(render::layer3(file, file, symbol, kind, &hunk))
                    }
                }
            }
            "pr" => {
                let pr_number = params.pr_number.ok_or("pr_number required for action=pr")?;
                let meta = github::pr_meta(&repo, pr_number).map_err(|e| e.to_string())?;
                let base = &meta.base_ref_name;
                let head = &meta.head_ref_name;
                let result = pipeline::run(&repo, base, head, threshold, false).await.map_err(|e| e.to_string())?;
                let repo_label = github::remote_name(&repo).unwrap_or_else(|_| params.repo.clone());
                let diff_output = render::layer1(&result, &repo_label, base, head);
                let ci = meta.status_check_rollup.as_deref().and_then(|checks| {
                    if checks.iter().any(|c| c.conclusion.as_deref() == Some("FAILURE")) { Some("fail") }
                    else if checks.iter().all(|c| c.conclusion.as_deref() == Some("SUCCESS")) { Some("pass") }
                    else { None }
                });
                let reviews = meta.reviews.as_deref().map(|r| r.len()).unwrap_or(0);
                Ok(render::pr_envelope(meta.number, &meta.title, &meta.body, ci, reviews, &diff_output))
            }
            "log" => {
                let range = require(&params.range, "range")?;
                let output = Command::new("git")
                    .args(["log", "--reverse", "--format=%H %s", range])
                    .current_dir(&repo)
                    .output()
                    .map_err(|e| e.to_string())?;
                if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).to_string());
                }
                let mut out = String::new();
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    if let Some((sha, subject)) = line.split_once(' ') {
                        let parent = format!("{sha}^");
                        let result = pipeline::run(&repo, &parent, sha, threshold, false).await.map_err(|e| e.to_string())?;
                        out.push_str(&format!("{} \"{}\"  +{}/−{}  {}f\n",
                            &sha[..7], subject,
                            result.summary.total_added, result.summary.total_removed, result.summary.file_count));
                        for file in &result.files {
                            out.push_str(&format!("  {}\n", render::file_line_only(file)));
                        }
                        out.push('\n');
                    }
                }
                Ok(out)
            }
            other => Err(format!("Unknown action: '{other}'. Use: diff | symbols | hunk | pr | log")),
        }
    }
}

fn require<'a>(opt: &'a Option<String>, name: &str) -> Result<&'a str, String> {
    opt.as_deref().ok_or_else(|| format!("'{name}' is required for this action"))
}

use std::process::Command;
