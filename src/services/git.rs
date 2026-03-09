// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::process::Command;

use crate::domain::{ChangeStatus, DiffStats, FileCategory, FileChange, StagedChanges};
use crate::error::{Error, Result};

pub struct GitService {
    repo: gix::Repository,
    work_dir: PathBuf,
}

impl GitService {
    pub fn discover() -> Result<Self> {
        let repo = gix::discover(".").map_err(|_| Error::NotAGitRepo)?;

        let work_dir = repo
            .workdir()
            .ok_or_else(|| Error::Git("Bare repository not supported".into()))?
            .to_path_buf();

        Ok(Self { repo, work_dir })
    }

    pub fn check_state(&self) -> Result<()> {
        let state = self.repo.state();
        if matches!(state, Some(gix::state::InProgress::Merge)) {
            return Err(Error::MergeInProgress);
        }
        Ok(())
    }

    // ─── Async Git Helpers ───

    async fn run_git(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.work_dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Git(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    // ─── Staged Changes (Single-Pass Diff) ───

    /// Returns `(StagedChanges, full_diff)` — the full diff is the raw unified
    /// diff output before per-file truncation, for use by the secret scanner.
    pub async fn get_staged_changes(
        &self,
        max_file_lines: usize,
    ) -> Result<(StagedChanges, String)> {
        self.check_state()?;

        // Two calls total: name-status (NUL delimited) + unified diff
        let (status_output, diff_output) = tokio::try_join!(
            self.run_git(&["diff", "-z", "--cached", "--name-status", "--no-renames"]),
            self.run_git(&[
                "diff",
                "--cached",
                "--no-ext-diff",
                "--unified=3",
                "--no-renames"
            ]),
        )?;

        let file_diffs = Self::split_unified_diff(&diff_output);

        let mut files = Vec::new();
        let mut stats = DiffStats::default();

        let mut parts = status_output.split('\0').filter(|s| !s.is_empty());

        while let Some(status_code) = parts.next() {
            let path_str = match parts.next() {
                Some(p) => p,
                None => break, // Should not happen with well-formed git output
            };

            let status = match status_code {
                "A" => ChangeStatus::Added,
                "M" => ChangeStatus::Modified,
                "D" => ChangeStatus::Deleted,
                _ => continue,
            };

            let file_path = PathBuf::from(path_str);
            let category = FileCategory::from_path(&file_path);
            let is_binary = Self::is_binary_path(&file_path);

            // For lookups in file_diffs, we need the string key.
            // Note: split_unified_diff currently uses paths from "diff --git a/... b/..." headers which are usually standard strings.
            // Complex unicode paths might mismatch if git output encoding differs, but -z guarantees strict bytes for status.
            let diff_key = file_path.to_string_lossy();

            // Count stats from full diff BEFORE truncation to get accurate numbers
            let full_diff = if is_binary {
                None
            } else {
                file_diffs.get(diff_key.as_ref())
            };

            let (additions, deletions) =
                full_diff.map(|d| Self::count_changes(d)).unwrap_or((0, 0));

            // Truncate diff for prompt context (binary files get empty diff)
            let diff = full_diff
                .map(|d| Self::truncate_diff(d, max_file_lines))
                .unwrap_or_default();

            files.push(FileChange {
                path: file_path,
                status,
                diff: Arc::new(diff),
                additions,
                deletions,
                category,
                is_binary,
            });

            stats.files_changed += 1;
            stats.insertions += additions;
            stats.deletions += deletions;
        }

        if files.is_empty() {
            return Err(Error::NoStagedChanges);
        }

        Ok((StagedChanges { files, stats }, diff_output))
    }

    /// Split a unified diff into per-file sections keyed by file path.
    fn split_unified_diff(diff: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();
        let mut current_path: Option<String> = None;
        let mut current_lines: Vec<&str> = Vec::new();

        for line in diff.lines() {
            if line.starts_with("diff --git ") {
                // Save previous file's accumulated diff
                if let Some(path) = current_path.take() {
                    result.insert(path, current_lines.join("\n"));
                }
                current_lines.clear();
            }

            // Extract path from +++ header (reliable for added/modified files)
            if let Some(path) = line.strip_prefix("+++ b/") {
                current_path = Some(path.to_string());
            }
            // For deleted files, +++ is /dev/null — use --- header instead
            if line == "+++ /dev/null"
                && let Some(last_minus) =
                    current_lines.iter().rev().find(|l| l.starts_with("--- a/"))
                && let Some(path) = last_minus.strip_prefix("--- a/")
            {
                current_path = Some(path.to_string());
            }

            current_lines.push(line);
        }

        // Don't forget the last file
        if let Some(path) = current_path {
            result.insert(path, current_lines.join("\n"));
        }

        result
    }

    fn truncate_diff(diff: &str, max_lines: usize) -> String {
        let lines: Vec<&str> = diff.lines().take(max_lines).collect();
        lines.join("\n")
    }

    // ─── File Content ───

    /// Fetch staged and HEAD content for multiple files concurrently.
    /// Spawns all git-show processes in parallel instead of sequentially.
    pub async fn fetch_file_contents(
        &self,
        paths: &[PathBuf],
    ) -> (HashMap<PathBuf, String>, HashMap<PathBuf, String>) {
        let mut set = tokio::task::JoinSet::new();

        for path in paths {
            let work_dir = self.work_dir.clone();
            let path = path.clone();
            set.spawn(async move {
                let staged =
                    Self::fetch_git_show(&work_dir, &format!(":0:{}", path.display())).await;
                let head =
                    Self::fetch_git_show(&work_dir, &format!("HEAD:{}", path.display())).await;
                (path, staged, head)
            });
        }

        let mut staged_map = HashMap::new();
        let mut head_map = HashMap::new();

        while let Some(result) = set.join_next().await {
            if let Ok((path, staged, head)) = result {
                if let Some(content) = staged {
                    staged_map.insert(path.clone(), content);
                }
                if let Some(content) = head {
                    head_map.insert(path, content);
                }
            }
        }

        (staged_map, head_map)
    }

    async fn fetch_git_show(work_dir: &Path, ref_path: &str) -> Option<String> {
        let output: std::process::Output = Command::new("git")
            .args(["show", ref_path])
            .current_dir(work_dir)
            .output()
            .await
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout).ok()
        } else {
            None
        }
    }

    // ─── Diff Parsing ───

    fn count_changes(diff: &str) -> (usize, usize) {
        let mut additions = 0;
        let mut deletions = 0;

        for line in diff.lines() {
            if line.starts_with('+') && !line.starts_with("+++") {
                additions += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                deletions += 1;
            }
        }

        (additions, deletions)
    }

    fn is_binary_path(path: &Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        matches!(
            ext,
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "ico"
                | "webp"
                | "woff"
                | "woff2"
                | "ttf"
                | "otf"
                | "zip"
                | "tar"
                | "gz"
                | "7z"
                | "pdf"
                | "exe"
                | "dll"
                | "so"
                | "dylib"
                | "mp3"
                | "mp4"
                | "wav"
        )
    }

    // ─── Staging Operations ───

    /// Check if any staged file also has unstaged modifications.
    /// Returns the list of overlapping file paths.
    pub async fn has_unstaged_overlap(&self) -> Result<Vec<PathBuf>> {
        let (staged_output, unstaged_output) = tokio::try_join!(
            self.run_git(&["diff", "--cached", "--name-only"]),
            self.run_git(&["diff", "--name-only"]),
        )?;

        let staged: std::collections::HashSet<&str> =
            staged_output.lines().filter(|l| !l.is_empty()).collect();
        let unstaged: std::collections::HashSet<&str> =
            unstaged_output.lines().filter(|l| !l.is_empty()).collect();

        Ok(staged.intersection(&unstaged).map(PathBuf::from).collect())
    }

    /// Unstage all currently staged files (soft reset).
    pub async fn unstage_all(&self) -> Result<()> {
        self.run_git(&["reset", "HEAD"]).await?;
        Ok(())
    }

    /// Stage specific files by path.
    pub async fn stage_files(&self, paths: &[PathBuf]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let path_strs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        let mut args: Vec<&str> = vec!["add", "--"];
        args.extend(path_strs.iter().map(|s| s.as_str()));

        self.run_git(&args).await?;
        Ok(())
    }

    // ─── Commit ───

    pub async fn commit(&self, message: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.work_dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Git(stderr.to_string()));
        }

        Ok(())
    }
}
