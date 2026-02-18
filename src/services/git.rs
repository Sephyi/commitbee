// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

    pub async fn get_staged_changes(&self, max_file_lines: usize) -> Result<StagedChanges> {
        self.check_state()?;

        // Two calls total (down from N+1): name-status + unified diff
        let (status_output, diff_output) = tokio::try_join!(
            self.run_git(&["diff", "--cached", "--name-status", "--no-renames"]),
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

        for line in status_output.lines() {
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() != 2 {
                continue;
            }

            let status = match parts[0] {
                "A" => ChangeStatus::Added,
                "M" => ChangeStatus::Modified,
                "D" => ChangeStatus::Deleted,
                _ => continue,
            };

            let file_path = Path::new(parts[1]).to_path_buf();
            let category = FileCategory::from_path(&file_path);
            let is_binary = Self::is_binary_path(&file_path);

            if is_binary {
                continue;
            }

            let diff = file_diffs
                .get(parts[1])
                .map(|d| Self::truncate_diff(d, max_file_lines))
                .unwrap_or_default();

            let (additions, deletions) = Self::count_changes(&diff);

            files.push(FileChange {
                path: file_path,
                status,
                diff,
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

        Ok(StagedChanges { files, stats })
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
            if line == "+++ /dev/null" {
                if let Some(last_minus) =
                    current_lines.iter().rev().find(|l| l.starts_with("--- a/"))
                {
                    if let Some(path) = last_minus.strip_prefix("--- a/") {
                        current_path = Some(path.to_string());
                    }
                }
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

    /// Get staged file content (from index)
    pub async fn get_staged_content(&self, path: &Path) -> Option<String> {
        let output: std::process::Output = Command::new("git")
            .args(["show", &format!(":0:{}", path.display())])
            .current_dir(&self.work_dir)
            .output()
            .await
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout).ok()
        } else {
            None
        }
    }

    /// Get HEAD file content
    pub async fn get_head_content(&self, path: &Path) -> Option<String> {
        let output: std::process::Output = Command::new("git")
            .args(["show", &format!("HEAD:{}", path.display())])
            .current_dir(&self.work_dir)
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
