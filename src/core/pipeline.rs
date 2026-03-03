// ABOUTME: Main analysis pipeline — classifies files, extracts symbols, detects splits.
// ABOUTME: Produces the FileDiff list consumed by the renderer.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use tokio::time::Duration;

use crate::core::cache::Cache;
use crate::core::config::Config;
use crate::core::diff::{DiffedSymbol, diff_symbols};
use crate::core::git::{self, FileStat, GitStatus};
use crate::core::lsp_confirmation::apply_lsp_confirmation;
use crate::core::skeletal;
use crate::core::{semantic, worktree};
use crate::languages::{Symbol, registry};

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
    pub symbols: Option<Vec<DiffedSymbol>>,
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
#[derive(Debug)]
pub struct DiffResult {
    pub summary: DiffSummary,
    pub files: Vec<FileDiff>,
}

/// Whether a file should have its symbols computed.
fn needs_expand(fd: &FileDiff, expand: bool, depth_symbols: bool, threshold: usize) -> bool {
    expand
        && (depth_symbols
            || match &fd.status {
                FileStatus::Split { .. } => true,
                FileStatus::Deleted { targets } if !targets.is_empty() => true,
                FileStatus::Modified => fd.added + fd.removed >= threshold,
                _ => false,
            })
}

/// Run the full diff pipeline.
///
/// `threshold`: line delta for auto-expanding symbols on M/D files.
/// `depth_symbols`: expand symbols for all files regardless of threshold.
/// `expand`: when false, suppress all symbol computation (file-level output only).
pub async fn run(
    repo: &Path,
    base: &str,
    head: &str,
    threshold: usize,
    depth_symbols: bool,
    expand: bool,
    config: &Config,
) -> Result<DiffResult> {
    // Resolve symbolic refs to SHAs so cache keys are stable across commits.
    let base_sha = git::resolve_ref(repo, base)?;
    let head_sha = git::resolve_ref(repo, head)?;

    let stats = git::diff_stats(repo, base, head)?;
    let mut cache = Cache::new(Cache::default_path());

    // Separate into categories for split detection
    let mut file_diffs = classify(&stats);

    // For files that look like they shrank significantly (large deletes, few adds),
    // check if new files contain their symbols — split detection.
    detect_splits(
        repo,
        &base_sha,
        &head_sha,
        &stats,
        &mut file_diffs,
        &mut cache,
    )?;

    // Auto-include symbols for S files and large M/D files
    for fd in &mut file_diffs {
        if needs_expand(fd, expand, depth_symbols, threshold) {
            let syms = if let Some(cached) = cache.get(repo, &base_sha, &head_sha, &fd.path) {
                cached
            } else {
                let syms = compute_symbols(repo, &base_sha, &head_sha, &fd.path)?;
                let _ = cache.put(repo, &base_sha, &head_sha, &fd.path, &syms);
                syms
            };
            fd.symbols = Some(syms);
        }
    }

    // LSP confirmation: try to upgrade ? → ! for each analyzed file.
    // Best-effort — falls back to heuristic output on timeout or unavailable server.
    if let Err(e) = confirm_with_lsp(repo, head, &mut file_diffs, config).await {
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

/// Group file diff indices by LSP command, in order of first appearance.
///
/// Files with unsupported extensions are skipped. Does not filter by availability —
/// callers decide whether to require the binary to be present.
fn build_lsp_groups(
    file_diffs: &[FileDiff],
) -> Vec<(Box<dyn crate::languages::LanguageSupport>, Vec<usize>)> {
    let mut groups: Vec<(Box<dyn crate::languages::LanguageSupport>, Vec<usize>)> = Vec::new();
    for (i, fd) in file_diffs.iter().enumerate() {
        let Some(ext) = std::path::Path::new(&fd.path)
            .extension()
            .and_then(|e| e.to_str())
        else {
            continue;
        };
        let Some(lang) = registry::for_extension(ext) else {
            continue;
        };
        let cmd = lang.lsp_command()[0];
        if let Some(group) = groups.iter_mut().find(|(l, _)| l.lsp_command()[0] == cmd) {
            group.1.push(i);
        } else {
            groups.push((lang, vec![i]));
        }
    }
    groups
}

/// Spawn an LSP server for a worktree, wait for initial indexing, and return the client.
async fn start_lsp(
    wt_path: &Path,
    lang: &dyn crate::languages::LanguageSupport,
    config: &Config,
) -> Result<semantic::LspClient> {
    lang.check_readiness(wt_path)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let indexing_timeout = config.lsp_indexing_timeout_for(lang.lsp_language_id());
    let mut client = semantic::LspClient::spawn(
        lang.lsp_command(),
        wt_path,
        lang.lsp_init_options(),
        config.lsp_timeout,
    )
    .await?;
    if let Err(e) = client.wait_for_indexing(indexing_timeout).await {
        tracing::debug!("LSP indexing wait failed: {e}");
    }
    Ok(client)
}

/// Try LSP confirmation for all files in file_diffs.
///
/// Creates one worktree at `head`, then spawns one LSP session per language family
/// present in the diff. Files of unsupported or unavailable languages are skipped.
async fn confirm_with_lsp(
    repo: &Path,
    head: &str,
    file_diffs: &mut [FileDiff],
    config: &Config,
) -> Result<()> {
    let groups: Vec<_> = build_lsp_groups(file_diffs)
        .into_iter()
        .filter(|(lang, _)| semantic::is_available(lang.lsp_command()[0]))
        .collect();

    if groups.is_empty() {
        return Ok(());
    }

    // One worktree for all language servers — creation is the expensive part.
    let wt = worktree::Worktree::create(repo, head)?;
    let wt_path = wt.path().to_path_buf();

    for (lang, indices) in groups {
        tracing::info!("starting {} for LSP confirmation...", lang.lsp_command()[0]);
        let mut client = match start_lsp(&wt_path, &*lang, config).await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("Failed to start {}: {e}", lang.lsp_command()[0]);
                continue;
            }
        };

        for idx in indices {
            let fd = &mut file_diffs[idx];
            let file_path = wt_path.join(&fd.path);
            if let Some(lsp_syms) = query_lsp_symbols(
                &mut client,
                &file_path,
                lang.lsp_language_id(),
                &fd.path,
                config.lsp_timeout,
            )
            .await
            {
                fd.confirmed = true;
                if let Some(syms) = &mut fd.symbols {
                    apply_lsp_confirmation(&lsp_syms, syms);
                }
            }
        }

        let _ = client.shutdown(config.lsp_timeout).await;
        tracing::info!("{} confirmation complete", lang.lsp_command()[0]);
    }

    Ok(())
}

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

/// Fetch and cache extracted symbols for a file at a single ref.
///
/// Key uses `git_ref` for both base and head slots to distinguish per-ref
/// symbol entries from cross-ref diff entries.
fn extract_cached(
    cache: &mut Cache,
    repo: &Path,
    git_ref: &str,
    path: &str,
) -> Option<Vec<Symbol>> {
    if let Some(cached) = cache.get::<Vec<Symbol>>(repo, git_ref, git_ref, path) {
        return Some(cached);
    }
    let bytes = git::file_at(repo, git_ref, path).ok()??;
    let syms = skeletal::extract(path, &bytes).ok()?;
    if !syms.is_empty() {
        let _ = cache.put(repo, git_ref, git_ref, path, &syms);
    }
    Some(syms)
}

/// Extract and cache symbols for a set of files at a given ref.
fn build_symbol_map(
    cache: &mut Cache,
    repo: &Path,
    git_ref: &str,
    stats: &[&FileStat],
) -> HashMap<String, Vec<Symbol>> {
    let mut map = HashMap::new();
    for s in stats {
        if let Some(syms) = extract_cached(cache, repo, git_ref, &s.path)
            && !syms.is_empty()
        {
            map.insert(s.path.clone(), syms);
        }
    }
    map
}

/// Heuristic split detection: if a deleted/large-shrunken file's symbols appear
/// in newly added files, mark the old file as Split and annotate the new files.
fn detect_splits(
    repo: &Path,
    base: &str,
    head: &str,
    stats: &[FileStat],
    file_diffs: &mut [FileDiff],
    cache: &mut Cache,
) -> Result<()> {
    // Find candidate source files: deleted or heavily-shrunken modified files
    let sources: Vec<&FileStat> = stats
        .iter()
        .filter(|s| {
            matches!(s.status, GitStatus::Deleted)
                || (matches!(s.status, GitStatus::Modified)
                    && s.removed > s.added * 2
                    && s.removed > 100)
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
    let source_symbols = build_symbol_map(cache, repo, base, &sources);
    let target_symbols = build_symbol_map(cache, repo, head, &targets);

    // For each source, check symbol overlap with each target
    for (src_path, src_syms) in &source_symbols {
        let src_names: HashSet<&str> = src_syms.iter().map(|s| s.name.as_str()).collect();

        let mut split_targets: Vec<String> = Vec::new();
        for (tgt_path, tgt_syms) in &target_symbols {
            let tgt_names: HashSet<&str> = tgt_syms.iter().map(|s| s.name.as_str()).collect();
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

/// Compute Layer 2 symbols for a file with LSP confirmation.
///
/// Equivalent to `compute_symbols` followed by a best-effort LSP confirmation pass.
/// Falls back silently to heuristic results if no LSP is available.
pub async fn symbols_confirmed(
    repo: &Path,
    base: &str,
    head: &str,
    path: &str,
    config: &Config,
) -> Result<Vec<DiffedSymbol>> {
    let mut syms = compute_symbols(repo, base, head, path)?;
    if let Err(e) = confirm_single_file(repo, head, path, &mut syms, config).await {
        tracing::debug!("LSP confirmation skipped for {path}: {e}");
    }
    Ok(syms)
}

/// Run LSP confirmation for a single file's symbol list.
async fn confirm_single_file(
    repo: &Path,
    head: &str,
    path: &str,
    syms: &mut [DiffedSymbol],
    config: &Config,
) -> Result<()> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .context("no file extension")?;

    let lang = registry::for_extension(ext).context("unsupported extension")?;
    let cmd = lang.lsp_command()[0];
    anyhow::ensure!(semantic::is_available(cmd), "{cmd} not available");

    let wt = worktree::Worktree::create(repo, head)?;
    let wt_path = wt.path().to_path_buf();

    let mut client = start_lsp(&wt_path, &*lang, config).await?;
    let file_path = wt_path.join(path);
    if let Some(lsp_syms) = query_lsp_symbols(
        &mut client,
        &file_path,
        lang.lsp_language_id(),
        path,
        config.lsp_timeout,
    )
    .await
    {
        apply_lsp_confirmation(&lsp_syms, syms);
    }

    let _ = client.shutdown(config.lsp_timeout).await;
    Ok(())
}

/// Compute Layer 2 symbols for a file.
pub fn compute_symbols(
    repo: &Path,
    base: &str,
    head: &str,
    path: &str,
) -> Result<Vec<DiffedSymbol>> {
    let base_bytes = git::file_at(repo, base, path)?.unwrap_or_default();
    let head_bytes = git::file_at(repo, head, path)?.unwrap_or_default();

    let base_syms = skeletal::extract(path, &base_bytes)?;
    let head_syms = skeletal::extract(path, &head_bytes)?;

    let base_map = HashMap::from([(path.to_string(), base_syms)]);
    let head_map = HashMap::from([(path.to_string(), head_syms)]);

    Ok(diff_symbols(&base_map, &head_map))
}

/// Open a file in the LSP and return its document symbols. Returns None on any error.
async fn query_lsp_symbols(
    client: &mut semantic::LspClient,
    file_path: &std::path::Path,
    language_id: &str,
    log_path: &str,
    timeout: Duration,
) -> Option<Vec<semantic::LspSymbol>> {
    if !file_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(file_path).ok()?;
    if let Err(e) = client.open_file(file_path, &content, language_id).await {
        tracing::debug!("LSP open_file failed for {log_path}: {e}");
        return None;
    }
    match client.document_symbols(file_path, timeout).await {
        Ok(syms) => {
            tracing::debug!(
                "LSP symbols for {log_path}: count={}, names=[{}]",
                syms.len(),
                syms.iter()
                    .map(|s| format!("{:?}:{}", s.kind, s.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            Some(syms)
        }
        Err(e) => {
            tracing::debug!("LSP documentSymbol failed for {log_path}: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_false_never_expands() {
        let cases = vec![
            FileDiff {
                status: FileStatus::Modified,
                path: "a.go".into(),
                added: 500,
                removed: 500,
                symbols: None,
                confirmed: false,
            },
            FileDiff {
                status: FileStatus::Split {
                    targets: vec!["b.go".into()],
                },
                path: "a.go".into(),
                added: 0,
                removed: 500,
                symbols: None,
                confirmed: false,
            },
            FileDiff {
                status: FileStatus::Deleted {
                    targets: vec!["b.go".into()],
                },
                path: "a.go".into(),
                added: 0,
                removed: 500,
                symbols: None,
                confirmed: false,
            },
        ];
        for fd in &cases {
            assert!(
                !needs_expand(fd, false, true, 0),
                "expand=false must suppress all expansion"
            );
        }
    }

    #[test]
    fn expand_true_respects_threshold_and_depth() {
        let large_modified = FileDiff {
            status: FileStatus::Modified,
            path: "a.go".into(),
            added: 150,
            removed: 100,
            symbols: None,
            confirmed: false,
        };
        let small_modified = FileDiff {
            status: FileStatus::Modified,
            path: "b.go".into(),
            added: 10,
            removed: 5,
            symbols: None,
            confirmed: false,
        };
        let split = FileDiff {
            status: FileStatus::Split {
                targets: vec!["c.go".into()],
            },
            path: "a.go".into(),
            added: 0,
            removed: 500,
            symbols: None,
            confirmed: false,
        };

        // Large modified exceeds threshold of 200
        assert!(needs_expand(&large_modified, true, false, 200));
        // Small modified does not
        assert!(!needs_expand(&small_modified, true, false, 200));
        // depth_symbols forces expansion regardless of size
        assert!(needs_expand(&small_modified, true, true, 200));
        // Split always expands when expand=true
        assert!(needs_expand(&split, true, false, 200));
    }

    #[test]
    fn lang_groups_are_distinct_per_lsp_command() {
        // Two Go files should share one group; one Rust file gets its own group.
        let file_diffs = vec![
            FileDiff {
                status: FileStatus::Modified,
                path: "main.go".to_string(),
                added: 10,
                removed: 5,
                symbols: None,
                confirmed: false,
            },
            FileDiff {
                status: FileStatus::Modified,
                path: "lib.go".to_string(),
                added: 3,
                removed: 1,
                symbols: None,
                confirmed: false,
            },
            FileDiff {
                status: FileStatus::Modified,
                path: "src/lib.rs".to_string(),
                added: 7,
                removed: 2,
                symbols: None,
                confirmed: false,
            },
        ];

        let groups = build_lsp_groups(&file_diffs);
        assert_eq!(groups.len(), 2, "Go and Rust should form two groups");
        assert_eq!(groups[0].1, vec![0, 1], "both Go files in first group");
        assert_eq!(groups[1].1, vec![2], "Rust file in second group");
    }
}
