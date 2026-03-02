// ABOUTME: MCP server exposing pasua operations as tools.
// ABOUTME: Identical operations to CLI; neither wraps the other.

use std::path::PathBuf;

use anyhow::Result;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::core::{github, hunk, pipeline, render};

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
                let result = pipeline::run(&repo, base, head, threshold, false)
                    .await
                    .map_err(|e| e.to_string())?;
                let repo_label = github::remote_name(&repo).unwrap_or_else(|_| params.repo.clone());
                Ok(render::layer1(&result, &repo_label, base, head))
            }
            "symbols" => {
                let base = require(&params.base, "base")?;
                let head = require(&params.head, "head")?;
                let file = require(&params.file, "file")?;
                let diffed = pipeline::compute_symbols(&repo, base, head, file)
                    .map_err(|e| e.to_string())?;
                Ok(render::layer2(file, &diffed))
            }
            "hunk" => {
                let base = require(&params.base, "base")?;
                let head = require(&params.head, "head")?;
                let file = require(&params.file, "file")?;
                let symbol = require(&params.symbol, "symbol")?;
                hunk::symbol_hunk(&repo, base, head, file, symbol).map_err(|e| e.to_string())
            }
            "pr" => {
                let pr_number = params.pr_number.ok_or("pr_number required for action=pr")?;
                let meta = github::pr_meta(&repo, pr_number).map_err(|e| e.to_string())?;
                let base = &meta.base_ref_name;
                let head = &meta.head_ref_name;
                let result = pipeline::run(&repo, base, head, threshold, false)
                    .await
                    .map_err(|e| e.to_string())?;
                let repo_label = github::remote_name(&repo).unwrap_or_else(|_| params.repo.clone());
                let diff_output = render::layer1(&result, &repo_label, base, head);
                let ci = meta.ci_status();
                let reviews = meta.reviews.as_deref().unwrap_or(&[]);
                Ok(render::pr_envelope(
                    meta.number,
                    &meta.title,
                    &meta.body,
                    &meta.state,
                    ci,
                    reviews,
                    &diff_output,
                ))
            }
            "log" => {
                let range = require(&params.range, "range")?;
                let commits = github::list_commits(&repo, range).map_err(|e| e.to_string())?;
                let mut out = String::new();
                for (sha, subject) in &commits {
                    let parent = format!("{sha}^");
                    let result = pipeline::run(&repo, &parent, sha, threshold, false)
                        .await
                        .map_err(|e| e.to_string())?;
                    out.push_str(&format!("{}\n", render::log_entry(sha, subject, &result)));
                    for file in &result.files {
                        out.push_str(&format!("  {}\n", render::file_line_only(file)));
                    }
                    out.push('\n');
                }
                Ok(out)
            }
            other => Err(format!(
                "Unknown action: '{other}'. Use: diff | symbols | hunk | pr | log"
            )),
        }
    }
}

fn require<'a>(opt: &'a Option<String>, name: &str) -> Result<&'a str, String> {
    opt.as_deref()
        .ok_or_else(|| format!("'{name}' is required for this action"))
}
