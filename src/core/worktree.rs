// ABOUTME: Temporary git worktree management for LSP analysis.
// ABOUTME: Creates a worktree at a given ref, removes it on drop.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A temporary git worktree at a specific ref.
///
/// Removed from the repo on drop.
pub struct Worktree {
    path: PathBuf,
    repo: PathBuf,
}

impl Worktree {
    /// Create a worktree at `git_ref` inside `<repo>/.git/pasua-worktrees/`.
    pub fn create(repo: &Path, git_ref: &str) -> Result<Self> {
        let dir_name = format!(
            "{}-{}",
            git_ref.replace(['/', '\\', ':'], "-"),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        );
        Self::create_at(repo, git_ref, &dir_name)
    }

    /// Create a worktree at `git_ref` with a specific directory name (used in tests).
    pub fn create_at(repo: &Path, git_ref: &str, dir_name: &str) -> Result<Self> {
        let path = repo.join(".git").join("pasua-worktrees").join(dir_name);

        // Remove stale worktree if it exists
        if path.exists() {
            let _ = Command::new("git")
                .args(["worktree", "remove", "--force", path.to_str().unwrap_or("")])
                .current_dir(repo)
                .output();
        }

        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "--detach",
                path.to_str().unwrap_or(""),
                git_ref,
            ])
            .current_dir(repo)
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "git worktree add failed for ref {git_ref}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(Self {
            path,
            repo: repo.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        let _ = Command::new("git")
            .args([
                "worktree",
                "remove",
                "--force",
                self.path.to_str().unwrap_or(""),
            ])
            .current_dir(&self.repo)
            .output();
    }
}
