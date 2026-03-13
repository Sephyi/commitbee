// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::PathBuf;
use std::sync::Arc;

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
        diff: Arc::from(diff),
        additions,
        deletions,
        category: FileCategory::from_path(&PathBuf::from(path)),
        is_binary: false,
        old_path: None,
        rename_similarity: None,
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
