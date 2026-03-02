// ABOUTME: GitHub integration via gh CLI subprocess and git commands.
// ABOUTME: Fetches PR metadata, file status, diffs, and file contents at refs.

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

/// Git-level status of a file in a diff.
#[derive(Debug, Clone, PartialEq)]
pub enum GitStatus {
    Added,
    Deleted,
    Modified,
    /// Renamed: (old_path, new_path, similarity_pct)
    Renamed(String, String, u8),
    /// Copied: (old_path, new_path, similarity_pct)
    Copied(String, String, u8),
}

/// A file's git status and line delta in a diff.
#[derive(Debug, Clone)]
pub struct FileStat {
    pub status: GitStatus,
    /// Primary path (new path for adds/renames, old path for deletes)
    pub path: String,
    pub added: usize,
    pub removed: usize,
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

/// List files changed between two refs with git status and line counts.
pub fn diff_stats(repo: &Path, base: &str, head: &str) -> Result<Vec<FileStat>> {
    // --name-status with -M gives rename detection
    let status_out = Command::new("git")
        .args([
            "diff",
            "--find-renames",
            "--name-status",
            &format!("{base}...{head}"),
        ])
        .current_dir(repo)
        .output()?;

    if !status_out.status.success() {
        anyhow::bail!(
            "git diff --name-status failed: {}",
            String::from_utf8_lossy(&status_out.stderr)
        );
    }

    // --numstat for line counts
    let numstat_out = Command::new("git")
        .args([
            "diff",
            "--find-renames",
            "--numstat",
            &format!("{base}...{head}"),
        ])
        .current_dir(repo)
        .output()?;

    if !numstat_out.status.success() {
        anyhow::bail!(
            "git diff --numstat failed: {}",
            String::from_utf8_lossy(&numstat_out.stderr)
        );
    }

    let statuses = parse_name_status(&String::from_utf8(status_out.stdout)?);
    let counts = parse_numstat(&String::from_utf8(numstat_out.stdout)?);

    // Merge: match by path
    let mut result = Vec::new();
    for stat in statuses {
        let (added, removed) = counts.get(&stat.path).copied().unwrap_or((0, 0));
        result.push(FileStat {
            status: stat.status,
            path: stat.path,
            added,
            removed,
        });
    }
    Ok(result)
}

struct RawStatus {
    status: GitStatus,
    path: String,
}

fn parse_name_status(output: &str) -> Vec<RawStatus> {
    let mut result = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.is_empty() {
            continue;
        }
        let code = parts[0];
        let stat = match code.chars().next() {
            Some('A') if parts.len() >= 2 => Some(RawStatus {
                status: GitStatus::Added,
                path: parts[1].to_string(),
            }),
            Some('D') if parts.len() >= 2 => Some(RawStatus {
                status: GitStatus::Deleted,
                path: parts[1].to_string(),
            }),
            Some('M') if parts.len() >= 2 => Some(RawStatus {
                status: GitStatus::Modified,
                path: parts[1].to_string(),
            }),
            Some('R') if parts.len() >= 3 => {
                let pct: u8 = code[1..].parse().unwrap_or(100);
                let old = parts[1].to_string();
                let new = parts[2].to_string();
                Some(RawStatus {
                    path: new.clone(),
                    status: GitStatus::Renamed(old, new, pct),
                })
            }
            Some('C') if parts.len() >= 3 => {
                let pct: u8 = code[1..].parse().unwrap_or(100);
                let old = parts[1].to_string();
                let new = parts[2].to_string();
                Some(RawStatus {
                    path: new.clone(),
                    status: GitStatus::Copied(old, new, pct),
                })
            }
            _ => None,
        };
        if let Some(s) = stat {
            result.push(s);
        }
    }
    result
}

fn parse_numstat(output: &str) -> std::collections::HashMap<String, (usize, usize)> {
    let mut map = std::collections::HashMap::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() != 3 {
            continue;
        }
        let added: usize = parts[0].parse().unwrap_or(0);
        let removed: usize = parts[1].parse().unwrap_or(0);
        // For renames, numstat shows "old => new" or just new path
        let path = parts[2].to_string();
        // Handle rename notation "{old => new}/file"
        let path = if path.contains('{') {
            normalize_rename_path(&path)
        } else {
            path
        };
        map.insert(path, (added, removed));
    }
    map
}

/// Normalize git's compact rename notation like "a/{old => new}/b" to "a/new/b"
fn normalize_rename_path(path: &str) -> String {
    if let (Some(open), Some(close)) = (path.find('{'), path.find('}')) {
        let prefix = &path[..open];
        let suffix = &path[close + 1..];
        let middle = &path[open + 1..close];
        let new_part = if let Some(arrow) = middle.find(" => ") {
            &middle[arrow + 4..]
        } else {
            middle
        };
        format!("{prefix}{new_part}{suffix}")
    } else {
        path.to_string()
    }
}

/// Get the raw unified diff for a single file between two refs.
pub fn file_diff(repo: &Path, base: &str, head: &str, file: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["diff", &format!("{base}...{head}"), "--", file])
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

/// Read file contents at a given ref. Returns None if file doesn't exist at that ref.
pub fn file_at(repo: &Path, git_ref: &str, file: &str) -> Result<Option<Vec<u8>>> {
    let output = Command::new("git")
        .args(["show", &format!("{git_ref}:{file}")])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    Ok(Some(output.stdout))
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
        .or_else(|| url.rsplit('/').nth(1).zip(url.rsplit('/').next()).map(|_| &url[..]))
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

    #[test]
    fn normalize_rename_path_basic() {
        assert_eq!(
            normalize_rename_path("cmd/{server => main}.go"),
            "cmd/main.go"
        );
    }

    #[test]
    fn normalize_rename_path_prefix_suffix() {
        assert_eq!(
            normalize_rename_path("tools/{registry => local}.go"),
            "tools/local.go"
        );
    }

    #[test]
    fn parse_name_status_basic() {
        let input = "A\tfoo.go\nD\tbar.go\nM\tbaz.go\nR90\told.go\tnew.go\n";
        let result = parse_name_status(input);
        assert_eq!(result.len(), 4);
        assert!(matches!(result[0].status, GitStatus::Added));
        assert!(matches!(result[1].status, GitStatus::Deleted));
        assert!(matches!(result[2].status, GitStatus::Modified));
        assert!(matches!(result[3].status, GitStatus::Renamed(_, _, 90)));
    }
}
