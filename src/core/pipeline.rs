// ABOUTME: Main analysis pipeline — classifies files, extracts symbols, detects splits.
// ABOUTME: Produces the FileDiff list consumed by the renderer.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use tokio::time::Duration;

use crate::core::cache::Cache;
use crate::core::github::{self, FileStat, GitStatus};
use crate::core::skeletal;
use crate::core::{semantic, worktree};
use crate::languages::{registry, Symbol};

/// Classified status of a file in the pasua output.
#[derive(Debug, Clone)]
pub enum FileStatus {
    Modified,
    Added {
        /// File it was split/moved from (heuristic)
        source: Option<String>,
    },
    Deleted {
        /// Files it was split into (heuristic)
        targets: Vec<String>,
    },
    /// File split into multiple targets
    Split {
        targets: Vec<String>,
    },
    /// File renamed or moved (git-detected)
    Renamed {
        old_path: String,
        new_path: String,
    },
}

/// A file entry in the diff, enriched with symbols if Layer 2 applies.
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub status: FileStatus,
    /// Display path (new path for adds/renames; old path for deletes)
    pub path: String,
    pub added: usize,
    pub removed: usize,
    /// Layer 2 symbols, if computed (None = not yet fetched)
    pub symbols: Option<Vec<crate::core::diff::DiffedSymbol>>,
    /// LSP confirmed (true) or heuristic (false)
    pub confirmed: bool,
}

/// Summary totals for the diff header.
#[derive(Debug)]
pub struct DiffSummary {
    pub total_added: usize,
    pub total_removed: usize,
    pub file_count: usize,
}

/// Result of a full diff analysis.
pub struct DiffResult {
    pub summary: DiffSummary,
    pub files: Vec<FileDiff>,
}

/// Default LSP timeout.
const LSP_TIMEOUT: Duration = Duration::from_secs(30);

/// Run the full Layer 1 pipeline.
///
/// `threshold`: line delta for auto-including Layer 2 on M/D files.
pub async fn run(repo: &Path, base: &str, head: &str, threshold: usize) -> Result<DiffResult> {
    let stats = github::diff_stats(repo, base, head)?;
    let cache = Cache::new(Cache::default_path());

    // Separate into categories for split detection
    let mut file_diffs = classify(&stats);

    // For files that look like they shrank significantly (large deletes, few adds),
    // check if new files contain their symbols — split detection.
    detect_splits(repo, base, head, &stats, &mut file_diffs)?;

    // Auto-include Layer 2 for S files and large M/D files
    for fd in &mut file_diffs {
        let needs_layer2 = match &fd.status {
            FileStatus::Split { .. } => true,
            FileStatus::Deleted { targets } if !targets.is_empty() => true,
            FileStatus::Modified => fd.added + fd.removed >= threshold,
            _ => false,
        };
        if needs_layer2 {
            let syms = if let Some(cached) = cache.get(repo, base, head, &fd.path) {
                cached
            } else {
                let syms = compute_symbols(repo, base, head, &fd.path, &fd.status)?;
                let _ = cache.put(repo, base, head, &fd.path, &syms);
                syms
            };
            fd.symbols = Some(syms);
        }
    }

    // LSP confirmation: try to upgrade ? → ! for each analyzed file.
    // Best-effort — falls back to heuristic output on timeout or unavailable server.
    if let Err(e) = confirm_with_lsp(repo, head, &mut file_diffs).await {
        tracing::debug!("LSP confirmation skipped: {e}");
    }

    let total_added = file_diffs.iter().map(|f| f.added).sum();
    let total_removed = file_diffs.iter().map(|f| f.removed).sum();
    let file_count = file_diffs.len();

    Ok(DiffResult {
        summary: DiffSummary {
            total_added,
            total_removed,
            file_count,
        },
        files: file_diffs,
    })
}

/// Try LSP confirmation for all files in file_diffs.
///
/// Creates a worktree at `head`, starts the language server, and uses
/// `textDocument/documentSymbol` to verify which symbols exist in each file.
/// Updates `confirmed = true` where the LSP agrees with the heuristic result.
async fn confirm_with_lsp(
    repo: &Path,
    head: &str,
    file_diffs: &mut Vec<FileDiff>,
) -> Result<()> {
    // Determine which file extensions are present and have a supported language
    let extensions: Vec<&str> = file_diffs
        .iter()
        .filter_map(|fd| {
            std::path::Path::new(&fd.path)
                .extension()
                .and_then(|e| e.to_str())
        })
        .collect();

    // Pick the first supported language with an available LSP server
    let lang = extensions
        .iter()
        .find_map(|ext| {
            let lang = registry::for_extension(ext)?;
            let cmd = lang.lsp_command()[0];
            if semantic::is_available(cmd) {
                Some(lang)
            } else {
                None
            }
        });

    let Some(lang) = lang else {
        return Ok(()); // No supported LSP available
    };

    eprintln!("pasua: starting {} for LSP confirmation...", lang.lsp_command()[0]);

    // Create a worktree at head for LSP to index
    let wt = worktree::Worktree::create(repo, head)?;
    let wt_path = wt.path().to_path_buf();

    let mut client = semantic::LspClient::spawn(lang.lsp_command(), &wt_path, LSP_TIMEOUT)
        .await
        .with_context(|| format!("Failed to start {}", lang.lsp_command()[0]))?;

    eprintln!("pasua: LSP server ready, confirming symbols...");

    // For each file that has symbols or is part of a split, verify via LSP
    for fd in file_diffs.iter_mut() {
        let file_path = wt_path.join(&fd.path);
        if !file_path.exists() {
            continue;
        }

        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Err(e) = client.open_file(&file_path, &content).await {
            tracing::debug!("LSP open_file failed for {}: {e}", fd.path);
            continue;
        }

        match client.document_symbols(&file_path, LSP_TIMEOUT).await {
            Ok(lsp_syms) => {
                let lsp_names: std::collections::HashSet<String> =
                    lsp_syms.iter().map(|s| s.name.clone()).collect();

                // Confirm file-level entry
                fd.confirmed = true;

                // Confirm individual symbols if we have Layer 2
                if let Some(syms) = &mut fd.symbols {
                    for sym in syms.iter_mut() {
                        sym.confirmed = lsp_names.contains(&sym.name);
                    }
                }
            }
            Err(e) => {
                tracing::debug!("LSP documentSymbol failed for {}: {e}", fd.path);
            }
        }
    }

    let _ = client.shutdown(LSP_TIMEOUT).await;
    eprintln!("pasua: LSP confirmation complete");

    Ok(())
}

use anyhow::Context;

fn classify(stats: &[FileStat]) -> Vec<FileDiff> {
    stats
        .iter()
        .map(|s| {
            let status = match &s.status {
                GitStatus::Added => FileStatus::Added { source: None },
                GitStatus::Deleted => FileStatus::Deleted { targets: vec![] },
                GitStatus::Modified => FileStatus::Modified,
                GitStatus::Renamed(old, new, _) => FileStatus::Renamed {
                    old_path: old.clone(),
                    new_path: new.clone(),
                },
                GitStatus::Copied(old, new, _) => FileStatus::Renamed {
                    old_path: old.clone(),
                    new_path: new.clone(),
                },
            };
            FileDiff {
                status,
                path: s.path.clone(),
                added: s.added,
                removed: s.removed,
                symbols: None,
                confirmed: false,
            }
        })
        .collect()
}

/// Heuristic split detection: if a deleted/large-shrunken file's symbols appear
/// in newly added files, mark the old file as Split and annotate the new files.
fn detect_splits(
    repo: &Path,
    base: &str,
    head: &str,
    stats: &[FileStat],
    file_diffs: &mut Vec<FileDiff>,
) -> Result<()> {
    // Find candidate source files: deleted or heavily-shrunken modified files
    let sources: Vec<&FileStat> = stats
        .iter()
        .filter(|s| {
            matches!(s.status, GitStatus::Deleted)
                || (matches!(s.status, GitStatus::Modified) && s.removed > s.added * 2 && s.removed > 100)
        })
        .collect();

    // Find candidate target files: newly added files
    let targets: Vec<&FileStat> = stats
        .iter()
        .filter(|s| matches!(s.status, GitStatus::Added))
        .collect();

    if sources.is_empty() || targets.is_empty() {
        return Ok(());
    }

    // Extract symbols from source files (at base) and target files (at head)
    let mut source_symbols: HashMap<String, Vec<Symbol>> = HashMap::new();
    for s in &sources {
        if let Ok(Some(bytes)) = github::file_at(repo, base, &s.path)
            && let Ok(syms) = skeletal::extract(&s.path, &bytes)
            && !syms.is_empty()
        {
            source_symbols.insert(s.path.clone(), syms);
        }
    }

    let mut target_symbols: HashMap<String, Vec<Symbol>> = HashMap::new();
    for t in &targets {
        if let Ok(Some(bytes)) = github::file_at(repo, head, &t.path)
            && let Ok(syms) = skeletal::extract(&t.path, &bytes)
            && !syms.is_empty()
        {
            target_symbols.insert(t.path.clone(), syms);
        }
    }

    // For each source, check symbol overlap with each target
    for (src_path, src_syms) in &source_symbols {
        let src_names: std::collections::HashSet<&str> =
            src_syms.iter().map(|s| s.name.as_str()).collect();

        let mut split_targets: Vec<String> = Vec::new();
        for (tgt_path, tgt_syms) in &target_symbols {
            let tgt_names: std::collections::HashSet<&str> =
                tgt_syms.iter().map(|s| s.name.as_str()).collect();
            let overlap = src_names.intersection(&tgt_names).count();
            // Threshold: at least 2 shared symbols, or >30% of source symbols present
            let significant = overlap >= 2 || (overlap > 0 && overlap * 100 / src_names.len() > 30);
            if significant {
                split_targets.push(tgt_path.clone());
            }
        }

        if split_targets.is_empty() {
            continue;
        }

        // Update source file to Split status
        if let Some(fd) = file_diffs.iter_mut().find(|f| f.path == *src_path) {
            fd.status = FileStatus::Split {
                targets: split_targets.clone(),
            };
        }

        // Annotate target files with their source
        for tgt in &split_targets {
            if let Some(fd) = file_diffs.iter_mut().find(|f| f.path == *tgt) {
                fd.status = FileStatus::Added {
                    source: Some(src_path.clone()),
                };
            }
        }
    }

    Ok(())
}

/// Compute Layer 2 symbols for a file.
fn compute_symbols(
    repo: &Path,
    base: &str,
    head: &str,
    path: &str,
    status: &FileStatus,
) -> Result<Vec<crate::core::diff::DiffedSymbol>> {
    let base_bytes = github::file_at(repo, base, path)?.unwrap_or_default();
    let head_bytes = github::file_at(repo, head, path)?.unwrap_or_default();

    let base_syms = skeletal::extract(path, &base_bytes)?;
    let head_syms = skeletal::extract(path, &head_bytes)?;

    let base_map = HashMap::from([(path.to_string(), base_syms)]);
    let head_map = HashMap::from([(path.to_string(), head_syms)]);

    let diffed = crate::core::diff::diff_symbols(&base_map, &head_map);
    Ok(diffed)
}
