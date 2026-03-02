// ABOUTME: GitHub API integration via the gh CLI subprocess.
// ABOUTME: Fetches PR metadata and resolves repo remote name.

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;
use std::process::{Command, Output};

/// Metadata for a pull request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrMeta {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub state: String,
    pub status_check_rollup: Option<Vec<StatusCheck>>,
    pub reviews: Option<Vec<Review>>,
}

#[derive(Debug, Deserialize)]
pub struct StatusCheck {
    pub state: Option<String>,
    pub conclusion: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Review {
    pub state: String,
}

impl PrMeta {
    /// Derive CI pass/fail from status check rollup. Returns None if mixed or absent.
    pub fn ci_status(&self) -> Option<&'static str> {
        self.status_check_rollup.as_deref().and_then(|checks| {
            if checks.iter().any(|c| {
                c.conclusion.as_deref() == Some("FAILURE")
                    || c.state.as_deref() == Some("FAILURE")
                    || c.state.as_deref() == Some("ERROR")
            }) {
                Some("fail")
            } else if checks.iter().any(|c| {
                c.state.as_deref() == Some("PENDING")
                    || (c.conclusion.is_none() && c.state.as_deref() != Some("SUCCESS"))
            }) {
                Some("pending")
            } else if checks.iter().all(|c| {
                c.conclusion.as_deref() == Some("SUCCESS") || c.state.as_deref() == Some("SUCCESS")
            }) {
                Some("pass")
            } else {
                None
            }
        })
    }
}

fn require_success(output: &Output, cmd: &str) -> Result<()> {
    if !output.status.success() {
        anyhow::bail!("{cmd} failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

/// Fetch PR metadata from GitHub.
pub fn pr_meta(repo: &Path, number: u64) -> Result<PrMeta> {
    let output = Command::new("gh")
        .args([
            "pr",
            "view",
            &number.to_string(),
            "--json",
            "number,title,body,baseRefName,headRefName,state,statusCheckRollup,reviews",
        ])
        .current_dir(repo)
        .output()?;

    require_success(&output, "gh pr view")?;

    let meta = serde_json::from_slice(&output.stdout)?;
    Ok(meta)
}

/// Get repo remote name, e.g. "owner/repo"
pub fn remote_name(repo: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        return Ok(String::new());
    }

    let url = String::from_utf8(output.stdout)?.trim().to_string();
    // Parse "git@github.com:owner/repo.git" or "https://github.com/owner/repo"
    let name = url
        .trim_end_matches(".git")
        .rsplit(':')
        .next()
        .or_else(|| {
            url.rsplit('/')
                .nth(1)
                .zip(url.rsplit('/').next())
                .map(|_| &url[..])
        })
        .unwrap_or(&url)
        .to_string();

    // Simplify to "owner/repo"
    let name = if let Some(stripped) = name.strip_prefix("https://github.com/") {
        stripped.to_string()
    } else if name.contains(':') {
        name.rsplit(':').next().unwrap_or(&name).to_string()
    } else {
        name
    };

    Ok(name)
}
