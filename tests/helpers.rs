// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::path::PathBuf;
use std::sync::Arc;

use commitbee::domain::{
    ChangeStatus, CodeSymbol, DiffStats, FileCategory, FileChange, StagedChanges, SymbolKind,
};

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

/// Create a renamed FileChange for testing
#[allow(dead_code)]
pub fn make_renamed_file(old_path: &str, new_path: &str, similarity: u8) -> FileChange {
    make_renamed_file_with_diff(old_path, new_path, similarity, "", 0, 0)
}

/// Create a renamed FileChange with a diff body and explicit add/delete counts.
///
/// Useful for splitter tests that exercise diff-shape grouping on renames.
#[allow(dead_code)]
pub fn make_renamed_file_with_diff(
    old_path: &str,
    new_path: &str,
    similarity: u8,
    diff: &str,
    additions: usize,
    deletions: usize,
) -> FileChange {
    FileChange {
        path: PathBuf::from(new_path),
        status: ChangeStatus::Renamed,
        diff: Arc::from(diff),
        additions,
        deletions,
        category: FileCategory::from_path(&PathBuf::from(new_path)),
        is_binary: false,
        old_path: Some(PathBuf::from(old_path)),
        rename_similarity: Some(similarity),
    }
}

/// Create a minimal `CodeSymbol` for testing, with `line: 1, end_line: 10`.
///
/// For tests that need the symbol to sit at a specific line range (e.g. to
/// exercise hunk-to-span mapping), use [`make_symbol_at`] instead.
#[allow(dead_code)]
pub fn make_symbol(
    name: &str,
    kind: SymbolKind,
    file: &str,
    is_public: bool,
    is_added: bool,
) -> CodeSymbol {
    make_symbol_at(name, kind, file, is_public, is_added, 1, 10)
}

/// Create a minimal `CodeSymbol` at an arbitrary line range.
///
/// Prefer this variant when a test needs to pin the symbol to specific
/// `line` / `end_line` positions (for example, to line up with a manually
/// crafted diff hunk). For the common case where positions are irrelevant,
/// [`make_symbol`] uses the defaults `line: 1, end_line: 10`.
#[allow(dead_code)]
pub fn make_symbol_at(
    name: &str,
    kind: SymbolKind,
    file: &str,
    is_public: bool,
    is_added: bool,
    line: usize,
    end_line: usize,
) -> CodeSymbol {
    CodeSymbol {
        kind,
        name: name.to_string(),
        file: PathBuf::from(file),
        line,
        end_line,
        is_public,
        is_added,
        is_whitespace_only: None,
        span_change_kind: None,
        signature: None,
        parent_scope: None,
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
