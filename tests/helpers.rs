// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use commitbee::domain::{ChangeStatus, DiffStats, FileCategory, FileChange, StagedChanges};

/// Create a minimal FileChange for testing
#[allow(dead_code)]
pub fn make_file_change(
    path: &str,
    status: ChangeStatus,
    diff: &str,
    additions: usize,
    deletions: usize,
) -> FileChange {
    FileChange {
        path: PathBuf::from(path),
        status,
        diff: diff.to_string(),
        additions,
        deletions,
        category: FileCategory::from_path(&PathBuf::from(path)),
        is_binary: false,
    }
}

/// Create StagedChanges from a list of FileChanges
#[allow(dead_code)]
pub fn make_staged_changes(files: Vec<FileChange>) -> StagedChanges {
    let insertions: usize = files.iter().map(|f| f.additions).sum();
    let deletions: usize = files.iter().map(|f| f.deletions).sum();
    StagedChanges {
        stats: DiffStats {
            files_changed: files.len(),
            insertions,
            deletions,
        },
        files,
    }
}
