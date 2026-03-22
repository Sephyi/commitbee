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
        end_line: 10,
        is_public,
        is_added,
        is_whitespace_only: None,
        signature: None,
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
