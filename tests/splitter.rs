// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

mod helpers;

use std::path::PathBuf;

use commitbee::domain::{ChangeStatus, CodeSymbol, SymbolKind};
use commitbee::services::splitter::{CommitSplitter, SplitSuggestion};
use helpers::{
    make_file_change, make_renamed_file, make_renamed_file_with_diff, make_staged_changes,
};

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
        end_line: 10,
        is_public,
        is_added,
        is_whitespace_only: None,
        span_change_kind: None,
        signature: None,
        parent_scope: None,
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

// ─── Diff-shape grouping tests ──────────────────────────────────────────────

#[test]
fn same_shape_changes_grouped_together() {
    // Three files with identical let-chain refactors (balanced adds/removes, indent-only)
    let indent_diff = "@@ -10,5 +10,4 @@\n-        if let Some(x) = foo() {\n-            if bar {\n-                do_thing();\n-            }\n-        }\n+        if let Some(x) = foo()\n+            && bar\n+        {\n+            do_thing();\n";
    let changes = make_staged_changes(vec![
        make_file_change("src/config.rs", ChangeStatus::Modified, indent_diff, 4, 5),
        make_file_change(
            "src/services/sanitizer.rs",
            ChangeStatus::Modified,
            indent_diff,
            4,
            5,
        ),
        make_file_change(
            "src/services/splitter.rs",
            ChangeStatus::Modified,
            indent_diff,
            4,
            5,
        ),
    ]);

    let result = CommitSplitter::analyze(&changes, &[]);
    // All three files have the same diff shape → should be one group → single commit
    assert!(
        matches!(result, SplitSuggestion::SingleCommit),
        "Files with identical diff shapes should be grouped together"
    );
}

#[test]
fn different_shape_changes_split() {
    // One file with a big feature addition, another with a small let-chain refactor
    let feature_diff = "@@ -0,0 +1,20 @@\n+pub fn new_function() {\n+    let x = 1;\n+    let y = 2;\n+    println!(\"{}\", x + y);\n+}\n";
    let refactor_diff = "@@ -10,3 +10,2 @@\n-        if let Some(x) = foo() {\n-            bar();\n-        }\n+        if let Some(x) = foo() { bar(); }\n";

    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/analyzer.rs",
            ChangeStatus::Modified,
            feature_diff,
            20,
            0,
        ),
        make_file_change(
            "src/services/sanitizer.rs",
            ChangeStatus::Modified,
            refactor_diff,
            2,
            3,
        ),
    ]);

    let symbols = vec![make_symbol(
        "new_function",
        SymbolKind::Function,
        "src/services/analyzer.rs",
        true,
        true,
    )];

    let result = CommitSplitter::analyze(&changes, &symbols);
    assert!(
        matches!(result, SplitSuggestion::SuggestSplit(_)),
        "Files with different diff shapes should be split"
    );
}

// ─── Support file separation tests ──────────────────────────────────────────

#[test]
fn docs_separated_from_source() {
    // Source files + doc files should produce separate groups
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/analyzer.rs",
            ChangeStatus::Modified,
            "@@ -1,3 +1,5 @@\n+use rayon;\n+use std::collections::HashMap;\n",
            20,
            5,
        ),
        make_file_change("README.md", ChangeStatus::Modified, "", 5, 2),
        make_file_change("CLAUDE.md", ChangeStatus::Modified, "", 10, 3),
    ]);

    let symbols = vec![make_symbol(
        "new_fn",
        SymbolKind::Function,
        "src/services/analyzer.rs",
        true,
        true,
    )];

    let result = CommitSplitter::analyze(&changes, &symbols);
    match result {
        SplitSuggestion::SuggestSplit(groups) => {
            // Docs should NOT be in the same group as source files
            let source_group = groups
                .iter()
                .find(|g| {
                    g.files
                        .iter()
                        .any(|f| f.to_string_lossy().contains("analyzer"))
                })
                .expect("should have source group");

            assert!(
                !source_group
                    .files
                    .iter()
                    .any(|f| f.to_string_lossy().ends_with(".md")),
                "Doc files should be in their own group, not dumped on source group"
            );
        }
        SplitSuggestion::SingleCommit => {
            panic!("Expected split between source and docs");
        }
    }
}

#[test]
fn config_separated_from_source() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/analyzer.rs",
            ChangeStatus::Modified,
            "@@ -1,3 +1,5 @@\n+use rayon;\n",
            10,
            2,
        ),
        make_file_change(
            "Cargo.toml",
            ChangeStatus::Modified,
            "@@ -30,1 +30,2 @@\n+rayon = \"1.11\"\n",
            3,
            1,
        ),
    ]);

    let symbols = vec![make_symbol(
        "new_fn",
        SymbolKind::Function,
        "src/services/analyzer.rs",
        true,
        true,
    )];

    let result = CommitSplitter::analyze(&changes, &symbols);
    match result {
        SplitSuggestion::SuggestSplit(groups) => {
            assert_eq!(groups.len(), 2, "Should have source and config groups");
        }
        SplitSuggestion::SingleCommit => {
            panic!("Expected split between source and config");
        }
    }
}

// ─── Symbol dependency merging tests ────────────────────────────────────────

#[test]
fn symbol_dependency_merges_groups() {
    // analyzer.rs adds extract_for_file, app.rs references it in its diff
    let analyzer_diff = "@@ -1,3 +1,5 @@\n+fn extract_for_file() {}\n+use rayon;\n";
    let app_diff =
        "@@ -10,5 +10,3 @@\n-let result = old_method();\n+let result = extract_for_file();\n";

    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/analyzer.rs",
            ChangeStatus::Modified,
            analyzer_diff,
            20,
            5,
        ),
        make_file_change("src/app.rs", ChangeStatus::Modified, app_diff, 3, 5),
    ]);

    let symbols = vec![
        make_symbol(
            "extract_for_file",
            SymbolKind::Function,
            "src/services/analyzer.rs",
            false,
            true,
        ),
        make_symbol(
            "old_method",
            SymbolKind::Function,
            "src/services/analyzer.rs",
            false,
            false,
        ),
    ];

    let result = CommitSplitter::analyze(&changes, &symbols);
    // app.rs mentions "extract_for_file" which is a symbol from analyzer.rs
    // → should be merged into same group → single commit
    assert!(
        matches!(result, SplitSuggestion::SingleCommit),
        "Files connected by symbol dependencies should be merged"
    );
}

#[test]
fn split_keeps_symbol_connected_files_together() {
    // File a.rs has added lines calling parse(), file b.rs defines parse()
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/a.rs",
            ChangeStatus::Modified,
            "-old\n+new(parse())",
            1,
            1,
        ),
        make_file_change(
            "src/b.rs",
            ChangeStatus::Modified,
            "-pub fn parse() {}\n+pub fn parse(s: &str) {}",
            1,
            1,
        ),
    ]);
    let symbols = vec![
        make_symbol("parse", SymbolKind::Function, "src/b.rs", true, true),
        make_symbol("parse", SymbolKind::Function, "src/b.rs", true, false),
    ];
    let result = CommitSplitter::analyze(&changes, &symbols);
    assert!(
        matches!(result, SplitSuggestion::SingleCommit),
        "files connected by symbol dependency should stay in one group"
    );
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

// ─── ChangeStatus::Deleted fixtures (audit D-037) ───────────────────────────

/// Baseline: a deleted file must not be silently dropped. Either the splitter
/// returns SingleCommit (where all files are implicitly in the one group) or
/// the deletion lands in exactly one split group.
#[test]
fn deleted_file_is_represented_in_splitter_output() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/llm/ollama.rs",
            ChangeStatus::Modified,
            "@@ -10,3 +10,3 @@\n-let old = 1;\n+let new = 2;\n",
            1,
            1,
        ),
        make_file_change(
            "src/services/llm/legacy.rs",
            ChangeStatus::Deleted,
            "@@ -1,10 +0,0 @@\n-pub fn retired() {}\n-pub fn also_retired() {}\n",
            0,
            10,
        ),
    ]);

    let result = CommitSplitter::analyze(&changes, &[]);
    let deleted_path = PathBuf::from("src/services/llm/legacy.rs");

    match &result {
        SplitSuggestion::SuggestSplit(groups) => {
            let occurrences = groups
                .iter()
                .filter(|g| g.files.contains(&deleted_path))
                .count();
            assert_eq!(
                occurrences, 1,
                "deleted file must appear in exactly one split group: {:?}",
                groups
            );
        }
        SplitSuggestion::SingleCommit => {
            // Single-commit means all files fall into the one implicit group
            // — the deleted file is still present in the input, so no further
            // assertion is needed beyond verifying the input shape.
            assert!(changes.files.iter().any(|f| f.path == deleted_path));
        }
    }
}

/// With a symbol-bearing addition in an unrelated module, the splitter is
/// likely to propose a split — the deletion must still land in exactly one
/// group (never duplicated, never dropped).
#[test]
fn deleted_file_is_placed_into_a_splitter_group() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/llm/anthropic.rs",
            ChangeStatus::Modified,
            "@@ -0,0 +1,20 @@\n+pub fn brand_new_api() {}\n",
            20,
            0,
        ),
        make_file_change(
            "src/services/sanitizer.rs",
            ChangeStatus::Deleted,
            "@@ -1,5 +0,0 @@\n-pub fn removed_sanitiser() {}\n",
            0,
            5,
        ),
    ]);

    let symbols = vec![make_symbol(
        "brand_new_api",
        SymbolKind::Function,
        "src/services/llm/anthropic.rs",
        true,
        true,
    )];

    let result = CommitSplitter::analyze(&changes, &symbols);
    let deleted_path = PathBuf::from("src/services/sanitizer.rs");

    match result {
        SplitSuggestion::SuggestSplit(groups) => {
            let occurrences = groups
                .iter()
                .filter(|g| g.files.contains(&deleted_path))
                .count();
            assert_eq!(
                occurrences, 1,
                "deleted file should appear in exactly one split group: groups={:?}",
                groups
            );
        }
        SplitSuggestion::SingleCommit => {
            // A single commit is also acceptable — but the deleted file must
            // have been surfaced via `changes.files` iteration (the splitter
            // does not drop files from the input), so we assert the input
            // invariant for clarity.
            assert!(changes.files.iter().any(|f| f.path == deleted_path));
        }
    }
}

// ─── ChangeStatus::Renamed fixtures (audit D-037) ───────────────────────────

/// The splitter must key off the *new* path (`path`) for module detection,
/// not the stale `old_path`. A rename + sibling modification under the same
/// new module should collapse into a single commit.
#[test]
fn renamed_file_grouped_by_new_path_module() {
    let changes = make_staged_changes(vec![
        make_renamed_file_with_diff(
            "src/services/old_module/helper.rs",
            "src/services/llm/helper.rs",
            95,
            "@@ -1,3 +1,3 @@\n-use old_name;\n+use new_name;\n",
            1,
            1,
        ),
        make_file_change(
            "src/services/llm/ollama.rs",
            ChangeStatus::Modified,
            "@@ -1,3 +1,3 @@\n-use old_name;\n+use new_name;\n",
            1,
            1,
        ),
    ]);

    let result = CommitSplitter::analyze(&changes, &[]);

    // Rename lives in the same module (`llm`) as the modification under its
    // new path — splitter should produce a single commit. This asserts the
    // splitter consults the new path (`path`) for module detection, not the
    // stale `old_path`.
    assert!(
        matches!(result, SplitSuggestion::SingleCommit),
        "renamed file under the same new-module as a sibling modification should group into a single commit, got {:?}",
        result
    );

    // Confirm old_path survived the pipeline unchanged (the splitter uses
    // `changes.subset` internally which clones FileChange records — so the
    // original `old_path` must still be present on the input).
    let renamed = changes
        .files
        .iter()
        .find(|f| f.status == ChangeStatus::Renamed)
        .expect("renamed file should be present");
    assert_eq!(
        renamed.old_path.as_deref(),
        Some(std::path::Path::new("src/services/old_module/helper.rs")),
        "old_path must be preserved verbatim",
    );
}

/// With a rename beside an unrelated addition, the renamed file must be
/// referenced by its *new* path in whichever group holds it, and `old_path`
/// / `rename_similarity` must round-trip unchanged through the splitter input.
#[test]
fn renamed_file_is_placed_into_a_splitter_group_with_old_path_preserved() {
    let rename = make_renamed_file("src/parser/old.rs", "src/parser/new.rs", 88);
    let changes = make_staged_changes(vec![
        rename,
        make_file_change(
            "src/services/llm/anthropic.rs",
            ChangeStatus::Modified,
            "@@ -0,0 +1,20 @@\n+pub fn brand_new_api() {}\n",
            20,
            0,
        ),
    ]);

    let symbols = vec![make_symbol(
        "brand_new_api",
        SymbolKind::Function,
        "src/services/llm/anthropic.rs",
        true,
        true,
    )];

    let result = CommitSplitter::analyze(&changes, &symbols);
    let renamed_path = PathBuf::from("src/parser/new.rs");

    // Whether the result is SingleCommit or SuggestSplit, the renamed file
    // must be represented by its *new* path in the splitter output, and the
    // original `old_path` must still be reachable via the input `changes`.
    match &result {
        SplitSuggestion::SuggestSplit(groups) => {
            let occurrences = groups
                .iter()
                .filter(|g| g.files.contains(&renamed_path))
                .count();
            assert_eq!(
                occurrences, 1,
                "renamed file should appear under its new path in exactly one group: {:?}",
                groups
            );
        }
        SplitSuggestion::SingleCommit => {
            assert!(changes.files.iter().any(|f| f.path == renamed_path));
        }
    }

    // old_path must be retrievable from the original StagedChanges the caller
    // passed in — the splitter borrows, it does not mutate, so this is a
    // regression guard rather than a behaviour assertion.
    let renamed = changes
        .files
        .iter()
        .find(|f| f.path == renamed_path)
        .expect("renamed file should be reachable via new path");
    assert_eq!(
        renamed.old_path.as_deref(),
        Some(std::path::Path::new("src/parser/old.rs")),
        "old_path for the rename must still be the original path",
    );
    assert_eq!(
        renamed.rename_similarity,
        Some(88),
        "rename_similarity must round-trip through the splitter input",
    );
}
