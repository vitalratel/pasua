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

/// Extract the remote name prefix from a ref like "oisee/main" → "oisee".
/// Returns None if the ref contains no slash.
fn infer_remote_from_ref(git_ref: &str) -> Option<&str> {
    let (candidate, _) = git_ref.split_once('/')?;
    Some(candidate)
}

/// Parse "owner/repo" from a git remote URL.
fn parse_remote_from_url(url: &str) -> String {
    let url = url.trim().trim_end_matches(".git");
    if let Some(stripped) = url.strip_prefix("https://github.com/") {
        return stripped.to_string();
    }
    // git@github.com:owner/repo
    if let Some(pos) = url.rfind(':') {
        return url[pos + 1..].to_string();
    }
    url.to_string()
}

/// List all configured remote names for the repo.
fn list_remotes(repo: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["remote"])
        .current_dir(repo)
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: vec![],
            stderr: vec![],
        });
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Get repo remote name, e.g. "owner/repo".
///
/// Uses base/head refs to infer the correct remote when they use "remote/branch" format
/// (e.g. "oisee/main"). Falls back to "origin" if no remote prefix is detected.
pub fn remote_name(repo: &Path, base: &str, head: &str) -> Result<String> {
    let remotes = list_remotes(repo);

    let remote = [base, head]
        .iter()
        .find_map(|r| {
            let candidate = infer_remote_from_ref(r)?;
            if remotes.contains(&candidate.to_string()) {
                Some(candidate.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "origin".to_string());

    let output = Command::new("git")
        .args(["remote", "get-url", &remote])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        return Ok(String::new());
    }

    let url = String::from_utf8(output.stdout)?;
    Ok(parse_remote_from_url(&url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_remote_from_ref_with_slash() {
        assert_eq!(infer_remote_from_ref("oisee/main"), Some("oisee"));
    }

    #[test]
    fn infer_remote_from_ref_without_slash() {
        assert_eq!(infer_remote_from_ref("main"), None);
    }

    #[test]
    fn parse_remote_from_url_ssh() {
        assert_eq!(
            parse_remote_from_url("git@github.com:oisee/vibing-steampunk.git"),
            "oisee/vibing-steampunk"
        );
    }

    #[test]
    fn parse_remote_from_url_https() {
        assert_eq!(
            parse_remote_from_url("https://github.com/oisee/vibing-steampunk"),
            "oisee/vibing-steampunk"
        );
    }

    #[test]
    fn parse_remote_from_url_https_with_git_suffix() {
        assert_eq!(
            parse_remote_from_url("https://github.com/oisee/vibing-steampunk.git"),
            "oisee/vibing-steampunk"
        );
    }

    #[test]
    fn infer_remote_from_ref_multi_segment() {
        // "oisee/feature/my-branch" → remote is "oisee", not "oisee/feature"
        assert_eq!(
            infer_remote_from_ref("oisee/feature/my-branch"),
            Some("oisee")
        );
    }
}
