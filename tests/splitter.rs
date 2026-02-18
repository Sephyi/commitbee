// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod helpers;

use std::path::PathBuf;

use commitbee::domain::{ChangeStatus, CodeSymbol, SymbolKind};
use commitbee::services::splitter::{CommitSplitter, SplitSuggestion};
use helpers::{make_file_change, make_staged_changes};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_symbol(
    name: &str,
    kind: SymbolKind,
    file: &str,
    is_public: bool,
    is_added: bool,
) -> CodeSymbol {
    CodeSymbol {
        kind,
        name: name.to_string(),
        file: PathBuf::from(file),
        line: 1,
        is_public,
        is_added,
    }
}

// ─── Split detection tests ───────────────────────────────────────────────────

#[test]
fn single_module_returns_single_commit() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/llm/ollama.rs",
            ChangeStatus::Modified,
            "",
            10,
            5,
        ),
        make_file_change(
            "src/services/llm/openai.rs",
            ChangeStatus::Modified,
            "",
            8,
            3,
        ),
    ]);
    let result = CommitSplitter::analyze(&changes, &[]);
    assert!(
        matches!(result, SplitSuggestion::SingleCommit),
        "Files in the same module should not suggest split"
    );
}

#[test]
fn two_modules_suggests_split() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/llm/anthropic.rs",
            ChangeStatus::Modified,
            "",
            20,
            5,
        ),
        make_file_change(
            "src/services/sanitizer.rs",
            ChangeStatus::Modified,
            "",
            3,
            1,
        ),
    ]);

    // Need symbols to differentiate types: llm group gets feat (new public fn),
    // sanitizer group gets fix (small change)
    let symbols = vec![make_symbol(
        "new_method",
        SymbolKind::Function,
        "src/services/llm/anthropic.rs",
        true,
        true,
    )];

    let result = CommitSplitter::analyze(&changes, &symbols);
    match result {
        SplitSuggestion::SuggestSplit(groups) => {
            assert_eq!(groups.len(), 2, "Should have 2 groups");
        }
        SplitSuggestion::SingleCommit => {
            panic!("Expected SuggestSplit for two different modules");
        }
    }
}

#[test]
fn all_test_files_returns_single_commit() {
    let changes = make_staged_changes(vec![
        make_file_change("tests/unit.rs", ChangeStatus::Modified, "", 10, 0),
        make_file_change("tests/integration.rs", ChangeStatus::Added, "", 20, 0),
    ]);
    let result = CommitSplitter::analyze(&changes, &[]);
    assert!(
        matches!(result, SplitSuggestion::SingleCommit),
        "All test files should not trigger split (no source modules)"
    );
}

#[test]
fn all_docs_files_returns_single_commit() {
    let changes = make_staged_changes(vec![
        make_file_change("README.md", ChangeStatus::Modified, "", 5, 2),
        make_file_change("CHANGELOG.md", ChangeStatus::Modified, "", 10, 3),
    ]);
    let result = CommitSplitter::analyze(&changes, &[]);
    assert!(
        matches!(result, SplitSuggestion::SingleCommit),
        "All docs files should not trigger split (no source modules)"
    );
}

#[test]
fn same_type_and_scope_returns_single_commit() {
    // Two source modules, but both infer the same type (fix) and no scope
    let changes = make_staged_changes(vec![
        make_file_change("src/config.rs", ChangeStatus::Modified, "", 2, 1),
        make_file_change("src/error.rs", ChangeStatus::Modified, "", 3, 2),
    ]);
    let result = CommitSplitter::analyze(&changes, &[]);
    assert!(
        matches!(result, SplitSuggestion::SingleCommit),
        "Same type+scope across groups should collapse to single commit"
    );
}

#[test]
fn test_file_attaches_to_matching_module() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/llm/anthropic.rs",
            ChangeStatus::Modified,
            "",
            20,
            5,
        ),
        make_file_change(
            "src/services/sanitizer.rs",
            ChangeStatus::Modified,
            "",
            3,
            1,
        ),
        make_file_change("tests/sanitizer.rs", ChangeStatus::Modified, "", 10, 0),
    ]);

    let symbols = vec![make_symbol(
        "new_method",
        SymbolKind::Function,
        "src/services/llm/anthropic.rs",
        true,
        true,
    )];

    let result = CommitSplitter::analyze(&changes, &symbols);
    match result {
        SplitSuggestion::SuggestSplit(groups) => {
            // Find the group containing sanitizer.rs
            let sanitizer_group = groups
                .iter()
                .find(|g| {
                    g.files
                        .iter()
                        .any(|f| f.to_string_lossy().contains("sanitizer.rs"))
                        && g.files.iter().any(|f| {
                            f.to_string_lossy().starts_with("src/")
                                && f.to_string_lossy().contains("sanitizer")
                        })
                })
                .expect("Should have a sanitizer group");

            // The test file should be in the same group as the source file
            assert!(
                sanitizer_group
                    .files
                    .iter()
                    .any(|f| f.to_string_lossy() == "tests/sanitizer.rs"),
                "tests/sanitizer.rs should attach to the sanitizer source group"
            );
        }
        SplitSuggestion::SingleCommit => {
            panic!("Expected SuggestSplit");
        }
    }
}

#[test]
fn groups_sorted_by_change_size() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/sanitizer.rs",
            ChangeStatus::Modified,
            "",
            3,
            1,
        ),
        make_file_change(
            "src/services/llm/anthropic.rs",
            ChangeStatus::Modified,
            "",
            50,
            20,
        ),
    ]);

    let symbols = vec![make_symbol(
        "new_method",
        SymbolKind::Function,
        "src/services/llm/anthropic.rs",
        true,
        true,
    )];

    let result = CommitSplitter::analyze(&changes, &symbols);
    if let SplitSuggestion::SuggestSplit(groups) = result {
        // Largest group should be first
        let first_has_anthropic = groups[0]
            .files
            .iter()
            .any(|f| f.to_string_lossy().contains("anthropic"));
        assert!(
            first_has_anthropic,
            "Largest change group should be sorted first"
        );
    }
}

// ─── Module detection tests ──────────────────────────────────────────────────

#[test]
fn detect_module_uses_parent_dir() {
    // Files under src/services/llm/ should group into "llm" module
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/llm/ollama.rs",
            ChangeStatus::Modified,
            "",
            5,
            2,
        ),
        make_file_change(
            "src/services/llm/anthropic.rs",
            ChangeStatus::Modified,
            "",
            5,
            2,
        ),
    ]);
    let result = CommitSplitter::analyze(&changes, &[]);
    // Both files should be in the same module ("llm"), so single commit
    assert!(matches!(result, SplitSuggestion::SingleCommit));
}

#[test]
fn detect_module_falls_back_to_stem_for_generic_dirs() {
    // Files directly under src/ should use file stem as module
    let changes = make_staged_changes(vec![
        make_file_change("src/config.rs", ChangeStatus::Modified, "", 5, 2),
        make_file_change("src/error.rs", ChangeStatus::Modified, "", 5, 2),
    ]);
    // config.rs and error.rs are in different "modules" (config vs error)
    // but with similar small changes, they'll likely get the same type (fix)
    // and no scope, so they'll collapse to SingleCommit
    let result = CommitSplitter::analyze(&changes, &[]);
    assert!(matches!(result, SplitSuggestion::SingleCommit));
}

// ─── StagedChanges::subset tests ─────────────────────────────────────────────

#[test]
fn subset_filters_correctly() {
    let changes = make_staged_changes(vec![
        make_file_change("src/a.rs", ChangeStatus::Modified, "", 10, 5),
        make_file_change("src/b.rs", ChangeStatus::Modified, "", 20, 3),
        make_file_change("src/c.rs", ChangeStatus::Added, "", 30, 0),
    ]);

    let subset = changes.subset(&[PathBuf::from("src/a.rs"), PathBuf::from("src/c.rs")]);

    assert_eq!(subset.files.len(), 2);
    assert_eq!(subset.stats.files_changed, 2);
    assert_eq!(subset.stats.insertions, 40); // 10 + 30
    assert_eq!(subset.stats.deletions, 5); // 5 + 0
}

#[test]
fn subset_empty_paths_returns_empty() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/a.rs",
        ChangeStatus::Modified,
        "",
        10,
        5,
    )]);

    let subset = changes.subset(&[]);
    assert!(subset.files.is_empty());
    assert_eq!(subset.stats.files_changed, 0);
}
