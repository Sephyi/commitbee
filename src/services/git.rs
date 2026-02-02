// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
// SPDX-License-Identifier: GPL-3.0-only

use std::path::{Path, PathBuf};

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
            .work_dir()
            .ok_or_else(|| Error::Git("Bare repository not supported".into()))?
            .to_path_buf();

        Ok(Self { repo, work_dir })
    }

    pub fn check_state(&self) -> Result<()> {
        // Check for merge/rebase in progress
        let state = self.repo.state();
        if matches!(state, Some(gix::state::InProgress::Merge)) {
            return Err(Error::MergeInProgress);
        }
        Ok(())
    }

    pub fn get_staged_changes(&self, max_file_lines: usize) -> Result<StagedChanges> {
        self.check_state()?;

        // Use git diff --cached --name-status to get list of staged files
        let output = std::process::Command::new("git")
            .args(["diff", "--cached", "--name-status", "--no-renames"])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Git(stderr.to_string()));
        }

        let mut files = Vec::new();
        let mut stats = DiffStats::default();

        let status_output = String::from_utf8_lossy(&output.stdout);

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
                continue; // Skip binary files
            }

            // Get diff content
            let diff = self.get_file_diff(&file_path, max_file_lines)?;
            let (additions, deletions) = Self::count_changes(&diff);

            files.push(FileChange {
                path: file_path,
                old_path: None,
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

    fn get_file_diff(&self, path: &Path, max_lines: usize) -> Result<String> {
        // Use git command for reliable diff output
        // --no-ext-diff: don't use external diff tools
        // --unified=3: standard 3 lines of context
        let output = std::process::Command::new("git")
            .args(["diff", "--cached", "--no-ext-diff", "--unified=3", "--"])
            .arg(path)
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Git(stderr.to_string()));
        }

        let diff = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = diff.lines().take(max_lines).collect();

        Ok(lines.join("\n"))
    }

    /// Get staged file content (from index)
    pub fn get_staged_content(&self, path: &Path) -> Option<String> {
        let output = std::process::Command::new("git")
            .args(["show", &format!(":0:{}", path.display())])
            .current_dir(&self.work_dir)
            .output()
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout).ok()
        } else {
            None
        }
    }

    /// Get HEAD file content
    pub fn get_head_content(&self, path: &Path) -> Option<String> {
        let output = std::process::Command::new("git")
            .args(["show", &format!("HEAD:{}", path.display())])
            .current_dir(&self.work_dir)
            .output()
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout).ok()
        } else {
            None
        }
    }

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

    pub fn commit(&self, message: &str) -> Result<()> {
        let output = std::process::Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Git(stderr.to_string()));
        }

        Ok(())
    }
}
