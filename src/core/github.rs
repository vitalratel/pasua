// ABOUTME: GitHub integration via gh CLI subprocess.
// ABOUTME: Fetches PR metadata, file lists, and ref resolution.

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

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

    if !output.status.success() {
        anyhow::bail!(
            "gh pr view failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let meta = serde_json::from_slice(&output.stdout)?;
    Ok(meta)
}

/// List files changed between two refs. Returns (path, added_lines, removed_lines).
pub fn changed_files(repo: &Path, base: &str, head: &str) -> Result<Vec<ChangedFile>> {
    let output = Command::new("git")
        .args([
            "diff",
            "--numstat",
            &format!("{base}...{head}"),
        ])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git diff --numstat failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut files = Vec::new();
    for line in String::from_utf8(output.stdout)?.lines() {
        // numstat format: added\tremoved\tpath
        // Binary files show "-\t-\tpath"
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() != 3 {
            continue;
        }
        let added: usize = parts[0].parse().unwrap_or(0);
        let removed: usize = parts[1].parse().unwrap_or(0);
        files.push(ChangedFile {
            path: parts[2].to_string(),
            added,
            removed,
        });
    }
    Ok(files)
}

/// A file changed in a diff.
#[derive(Debug)]
pub struct ChangedFile {
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

/// Get the raw unified diff for a single file between two refs.
pub fn file_diff(repo: &Path, base: &str, head: &str, file: &str) -> Result<String> {
    let output = Command::new("git")
        .args([
            "diff",
            &format!("{base}...{head}"),
            "--",
            file,
        ])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8(output.stdout)?)
}

/// Read file contents at a given ref.
pub fn file_at(repo: &Path, git_ref: &str, file: &str) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(["show", &format!("{git_ref}:{file}")])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git show failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(output.stdout)
}

/// Resolve a ref to its full commit SHA.
pub fn resolve_ref(repo: &Path, git_ref: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", git_ref])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn pasua_repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn resolve_head_ref() {
        let repo = pasua_repo();
        let sha = resolve_ref(&repo, "HEAD").unwrap();
        assert_eq!(sha.len(), 40, "expected full SHA, got: {sha}");
    }
}
