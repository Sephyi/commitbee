// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

mod helpers;

use std::path::PathBuf;

use commitbee::classify_diff_span;
use commitbee::config::Config;
use commitbee::domain::{
    ChangeStatus, CodeSymbol, CommitType, FileCategory, IntentKind, SymbolKind,
};
use commitbee::services::context::ContextBuilder;
use helpers::{make_file_change, make_renamed_file, make_staged_changes};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn default_config() -> Config {
    Config::default()
}

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

// ─── CommitType inference ─────────────────────────────────────────────────────

#[test]
fn infer_type_all_docs() {
    let changes = make_staged_changes(vec![
        make_file_change("README.md", ChangeStatus::Modified, "", 5, 2),
        make_file_change("CHANGELOG.md", ChangeStatus::Modified, "", 3, 1),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Docs,
        "all .md files should infer Docs"
    );
}

#[test]
fn infer_type_all_tests() {
    let changes = make_staged_changes(vec![
        make_file_change("tests/unit.rs", ChangeStatus::Modified, "", 10, 0),
        make_file_change("tests/integration.rs", ChangeStatus::Added, "", 20, 0),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Test,
        "all test files should infer Test"
    );
}

#[test]
fn infer_type_all_config() {
    let changes = make_staged_changes(vec![make_file_change(
        "Cargo.toml",
        ChangeStatus::Modified,
        "",
        3,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Chore,
        "Cargo.toml only should infer Chore"
    );
}

#[test]
fn infer_type_all_build() {
    let changes = make_staged_changes(vec![make_file_change(
        ".github/workflows/ci.yml",
        ChangeStatus::Modified,
        "",
        5,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Build,
        ".github/workflows/ci.yml should infer Build"
    );
}

#[test]
fn infer_type_new_public_symbols_is_feat() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/services/new_service.rs",
        ChangeStatus::Modified,
        "+pub fn new_service() {}",
        5,
        1,
    )]);
    let symbols = vec![make_symbol(
        "new_service",
        SymbolKind::Function,
        "src/services/new_service.rs",
        true,
        true,
    )];
    let ctx = ContextBuilder::build(&changes, &symbols, &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Feat,
        "a newly added public function should infer Feat"
    );
}

#[test]
fn infer_type_majority_new_files_is_feat() {
    // 2 Added, 1 Modified → majority new (>50%)
    let changes = make_staged_changes(vec![
        make_file_change("src/services/foo.rs", ChangeStatus::Added, "", 50, 0),
        make_file_change("src/services/bar.rs", ChangeStatus::Added, "", 30, 0),
        make_file_change("src/lib.rs", ChangeStatus::Modified, "", 5, 2),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Feat,
        "majority new files (>50%) should infer Feat"
    );
}

#[test]
fn infer_type_small_balanced_change_is_style() {
    // <20 insertions and <20 deletions, balanced, no symbols -> style
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-let x = 1;\n+let x = 2;",
        1,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Style,
        "small balanced change with no symbols should infer Style"
    );
}

#[test]
fn infer_type_small_unbalanced_change_is_refactor() {
    // <20 insertions and <20 deletions, unbalanced (more dels), no symbols -> refactor
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-removed line\n-another\n+replacement;",
        1,
        10,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Refactor,
        "small unbalanced change with no symbols should infer Refactor"
    );
}

// ─── Scope inference ─────────────────────────────────────────────────────────

#[test]
fn infer_scope_single_module() {
    let changes = make_staged_changes(vec![
        make_file_change("src/services/context.rs", ChangeStatus::Modified, "", 5, 2),
        make_file_change("src/services/git.rs", ChangeStatus::Modified, "", 3, 1),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_scope,
        Some("services".to_string()),
        "files in src/services/ should yield scope 'services'"
    );
}

#[test]
fn infer_scope_none_for_mixed_modules() {
    let changes = make_staged_changes(vec![
        make_file_change("src/services/context.rs", ChangeStatus::Modified, "", 5, 2),
        make_file_change("src/domain/change.rs", ChangeStatus::Modified, "", 3, 1),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.suggested_scope.is_none(),
        "files from different modules should yield no scope, got {:?}",
        ctx.suggested_scope
    );
}

// ─── Prompt output ───────────────────────────────────────────────────────────

#[test]
fn prompt_includes_symbols_when_present() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/services/context.rs",
        ChangeStatus::Modified,
        "+pub fn new_func() {}\n-fn old_func() {}",
        5,
        3,
    )]);
    let symbols = vec![
        make_symbol(
            "new_func",
            SymbolKind::Function,
            "src/services/context.rs",
            true,
            true,
        ),
        make_symbol(
            "old_func",
            SymbolKind::Function,
            "src/services/context.rs",
            false,
            false,
        ),
    ];
    let ctx = ContextBuilder::build(&changes, &symbols, &[], &default_config());
    let prompt = ctx.to_prompt();

    assert!(
        prompt.contains("SYMBOLS CHANGED:"),
        "prompt should contain 'SYMBOLS CHANGED:' when symbols are present"
    );
    assert!(
        prompt.contains("new_func"),
        "prompt should contain added symbol name 'new_func'"
    );
    assert!(
        prompt.contains("old_func"),
        "prompt should contain removed symbol name 'old_func'"
    );
}

#[test]
fn prompt_omits_symbols_when_empty() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-let x = 1;\n+let x = 2;",
        1,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();

    assert!(
        !prompt.contains("SYMBOLS CHANGED:"),
        "prompt should not contain 'SYMBOLS CHANGED:' when no symbols are present"
    );
}

// ─── Budget management ───────────────────────────────────────────────────────

#[test]
fn prompt_respects_budget() {
    // Generate a huge diff (10 000 lines)
    let huge_diff = "+ added line of code here\n".repeat(10_000);

    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        &huge_diff,
        10_000,
        0,
    )]);

    let mut config = default_config();
    config.max_context_chars = 5_000;

    let ctx = ContextBuilder::build(&changes, &[], &[], &config);
    let prompt = ctx.to_prompt();

    assert!(
        prompt.len() < 10_000,
        "prompt length {} should be less than 10 000 chars when budget is 5 000",
        prompt.len()
    );
}

// ─── FileCategory classification ─────────────────────────────────────────────

#[test]
fn file_category_source() {
    let cases = [
        ("src/main.rs", FileCategory::Source),
        ("lib/utils.ts", FileCategory::Source),
        ("app/module.py", FileCategory::Source),
        ("cmd/server.go", FileCategory::Source),
    ];

    for (path, expected) in cases {
        let got = FileCategory::from_path(&PathBuf::from(path));
        assert_eq!(got, expected, "{} should be classified as Source", path);
    }
}

#[test]
fn file_category_test() {
    let cases = [
        "tests/unit.rs",
        "src/foo_test.rs",
        "app/app.test.ts",
        "lib/lib_spec.js",
    ];

    for path in cases {
        let got = FileCategory::from_path(&PathBuf::from(path));
        assert_eq!(
            got,
            FileCategory::Test,
            "{} should be classified as Test, got {:?}",
            path,
            got
        );
    }
}

#[test]
fn file_category_docs() {
    let cases = [
        ("README.md", FileCategory::Docs),
        ("docs/guide.rst", FileCategory::Docs),
    ];

    for (path, expected) in cases {
        let got = FileCategory::from_path(&PathBuf::from(path));
        assert_eq!(got, expected, "{} should be classified as Docs", path);
    }
}

#[test]
fn file_category_config() {
    let cases = [
        ("Cargo.toml", FileCategory::Config),
        ("package.json", FileCategory::Config),
        (".gitignore", FileCategory::Config),
    ];

    for (path, expected) in cases {
        let got = FileCategory::from_path(&PathBuf::from(path));
        assert_eq!(got, expected, "{} should be classified as Config", path);
    }
}

#[test]
fn file_category_build() {
    let cases = [
        (".github/workflows/ci.yml", FileCategory::Build),
        ("Dockerfile", FileCategory::Build),
        ("Makefile", FileCategory::Build),
    ];

    for (path, expected) in cases {
        let got = FileCategory::from_path(&PathBuf::from(path));
        assert_eq!(got, expected, "{} should be classified as Build", path);
    }
}

// ─── Additional CommitType inference ──────────────────────────────────────────

#[test]
fn infer_type_more_deletions_is_refactor() {
    // 50 deletions > 10 insertions * 2 = 20
    let changes = make_staged_changes(vec![make_file_change(
        "src/services/old_module.rs",
        ChangeStatus::Modified,
        "",
        10,
        50,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Refactor,
        "deletions > insertions*2 should infer Refactor"
    );
}

#[test]
fn infer_type_default_fallback_is_refactor() {
    // 30 insertions, 30 deletions (not small, not deletion-heavy, not special category)
    let changes = make_staged_changes(vec![make_file_change(
        "src/services/module.rs",
        ChangeStatus::Modified,
        "",
        30,
        30,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Refactor,
        "large non-special change should fallback to Refactor (safer than Feat)"
    );
}

// ─── Additional budget and truncation ─────────────────────────────────────────

#[test]
fn symbols_budget_truncation() {
    // Create 20 symbols, with a very small budget
    let symbols: Vec<CodeSymbol> = (0..20)
        .map(|i| {
            make_symbol(
                &format!("function_{}", i),
                SymbolKind::Function,
                "src/lib.rs",
                true,
                true,
            )
        })
        .collect();

    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "",
        100,
        0,
    )]);

    let mut config = default_config();
    // Very small budget to force truncation
    config.max_context_chars = 500;

    let ctx = ContextBuilder::build(&changes, &symbols, &[], &config);
    let prompt = ctx.to_prompt();

    assert!(
        prompt.contains("more symbols"),
        "prompt should indicate truncated symbols when budget is exceeded"
    );
}

#[test]
fn skip_content_lock_files() {
    let changes = make_staged_changes(vec![make_file_change(
        "Cargo.lock",
        ChangeStatus::Modified,
        "+lots of lock file content\n".repeat(100).as_str(),
        100,
        50,
    )]);

    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.truncated_diff.contains("lock file - content skipped"),
        "lock file diff should contain skip message, got: {}",
        &ctx.truncated_diff[..ctx.truncated_diff.len().min(200)]
    );
}

// ─── Additional scope inference ──────────────────────────────────────────────

#[test]
fn scope_from_packages_prefix() {
    let changes = make_staged_changes(vec![make_file_change(
        "packages/foo/src/bar.rs",
        ChangeStatus::Modified,
        "",
        5,
        2,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(
        ctx.suggested_scope,
        Some("foo".to_string()),
        "packages/foo/src/bar.rs should yield scope 'foo'"
    );
}

// ─── Additional FileCategory classification ──────────────────────────────────

#[test]
fn file_category_other() {
    let path = PathBuf::from("data/file.xyz");
    let got = FileCategory::from_path(&path);
    assert_eq!(
        got,
        FileCategory::Other,
        "unknown extension .xyz should be classified as Other"
    );
}

// ─── Evidence flags ─────────────────────────────────────────────────────────

#[test]
fn evidence_mechanical_transform_balanced_no_symbols() {
    // Small balanced change, no symbols → mechanical
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-    old_indent\n+old_indent",
        5,
        5,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.is_mechanical,
        "balanced small change with no symbols should be mechanical"
    );
}

#[test]
fn evidence_not_mechanical_with_symbols() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+pub fn new_func() {}",
        5,
        3,
    )]);
    let symbols = vec![make_symbol(
        "new_func",
        SymbolKind::Function,
        "src/lib.rs",
        true,
        true,
    )];
    let ctx = ContextBuilder::build(&changes, &symbols, &[], &default_config());
    assert!(
        !ctx.is_mechanical,
        "change with new symbols should not be mechanical"
    );
}

#[test]
fn evidence_not_mechanical_large_change() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "",
        50,
        50,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        !ctx.is_mechanical,
        "large change (100 lines total) should not be mechanical"
    );
}

#[test]
fn evidence_bug_evidence_from_fix_comment() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+// fix: handle edge case where input is empty\n+if input.is_empty() { return; }",
        2,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.has_bug_evidence,
        "diff with '// fix' comment should have bug evidence"
    );
}

#[test]
fn evidence_no_bug_evidence_for_refactor() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-    if let Some(x) = foo() {\n-        bar();\n-    }\n+    if let Some(x) = foo() { bar(); }",
        1,
        3,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        !ctx.has_bug_evidence,
        "refactor without fix/bug comments should not have bug evidence"
    );
}

#[test]
fn evidence_dependency_only() {
    let changes = make_staged_changes(vec![
        make_file_change("Cargo.toml", ChangeStatus::Modified, "", 3, 1),
        make_file_change(".github/workflows/ci.yml", ChangeStatus::Modified, "", 2, 1),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.is_dependency_only,
        "all config/build files should be dependency_only"
    );
}

#[test]
fn evidence_not_dependency_only_with_source() {
    let changes = make_staged_changes(vec![
        make_file_change("Cargo.toml", ChangeStatus::Modified, "", 3, 1),
        make_file_change("src/lib.rs", ChangeStatus::Modified, "", 5, 2),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        !ctx.is_dependency_only,
        "mix of config and source should not be dependency_only"
    );
}

#[test]
fn evidence_public_api_removed_count() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-pub fn old_api() {}\n-pub fn another_old() {}",
        0,
        10,
    )]);
    let symbols = vec![
        make_symbol("old_api", SymbolKind::Function, "src/lib.rs", true, false),
        make_symbol(
            "another_old",
            SymbolKind::Function,
            "src/lib.rs",
            true,
            false,
        ),
    ];
    let ctx = ContextBuilder::build(&changes, &symbols, &[], &default_config());
    assert_eq!(
        ctx.public_api_removed_count, 2,
        "should count 2 removed public symbols"
    );
}

#[test]
fn prompt_contains_evidence_section() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-old\n+new",
        1,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("EVIDENCE:"),
        "prompt should contain EVIDENCE section"
    );
    assert!(
        prompt.contains("mechanical/formatting change?"),
        "prompt should contain mechanical transform question"
    );
    assert!(
        prompt.contains("bug-fix comments?"),
        "prompt should contain bug-fix question"
    );
}

#[test]
fn prompt_contains_constraints_when_no_bug_evidence() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-old\n+new",
        1,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("CONSTRAINTS (must follow):"),
        "prompt should contain CONSTRAINTS section when bug_evidence=no"
    );
    assert!(
        prompt.contains("No bug-fix comments found"),
        "prompt should mention no bug-fix constraint"
    );
}

// ─── API replacement type inference ─────────────────────────────────────────

#[test]
fn api_replacement_infers_refactor() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/services/context.rs",
        ChangeStatus::Modified,
        "+pub fn new_builder()\n-pub fn old_builder()",
        20,
        15,
    )]);
    let symbols = vec![
        make_symbol(
            "new_builder",
            SymbolKind::Function,
            "src/services/context.rs",
            true,
            true,
        ),
        make_symbol(
            "old_builder",
            SymbolKind::Function,
            "src/services/context.rs",
            true,
            false,
        ),
    ];
    let commit_type = ContextBuilder::infer_commit_type(&changes, &symbols, false, false);
    assert_eq!(
        commit_type,
        CommitType::Refactor,
        "adding new public API while removing old public API should be refactor, not feat"
    );
}

#[test]
fn api_addition_without_removal_infers_feat() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/services/context.rs",
        ChangeStatus::Modified,
        "+pub fn new_feature()",
        20,
        0,
    )]);
    let symbols = vec![make_symbol(
        "new_feature",
        SymbolKind::Function,
        "src/services/context.rs",
        true,
        true,
    )];
    let commit_type = ContextBuilder::infer_commit_type(&changes, &symbols, false, false);
    assert_eq!(
        commit_type,
        CommitType::Feat,
        "adding new public API without removing old ones should be feat"
    );
}

// ─── Prompt content tests ───────────────────────────────────────────────────

#[test]
fn prompt_includes_subject_budget() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-old\n+new",
        1,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();
    // Should contain "under XX chars" for subject budget
    assert!(
        prompt.contains("under ") && prompt.contains(" chars"),
        "prompt should contain subject character budget"
    );
}

#[test]
fn prompt_breaking_constraint_includes_description_guidance() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-pub fn old()\n+fn new()",
        1,
        1,
    )]);
    let symbols = vec![make_symbol(
        "old",
        SymbolKind::Function,
        "src/lib.rs",
        true,
        false,
    )];
    let ctx = ContextBuilder::build(&changes, &symbols, &[], &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("describe what was removed"),
        "breaking change constraint should guide the model to describe the removal"
    );
}

#[test]
fn prompt_evidence_uses_natural_language() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-old\n+new",
        1,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();
    // Should NOT contain snake_case internal identifiers
    assert!(
        !prompt.contains("mechanical_transform:"),
        "prompt should not use snake_case field names (mechanical_transform)"
    );
    assert!(
        !prompt.contains("bug_evidence:"),
        "prompt should not use snake_case field names (bug_evidence)"
    );
    assert!(
        !prompt.contains("- public_api_removed:"),
        "prompt should not use snake_case field names (public_api_removed)"
    );
}

// ─── Cross-project file categorization ──────────────────────────────────────

#[test]
fn file_category_csharp_is_source() {
    assert_eq!(
        FileCategory::from_path(std::path::Path::new("src/Models/User.cs")),
        FileCategory::Source
    );
}

#[test]
fn file_category_ruby_is_source() {
    assert_eq!(
        FileCategory::from_path(std::path::Path::new("app/models/user.rb")),
        FileCategory::Source
    );
}

#[test]
fn file_category_biome_is_config() {
    assert_eq!(
        FileCategory::from_path(std::path::Path::new("biome.json")),
        FileCategory::Config
    );
}

#[test]
fn file_category_dotfile_config() {
    assert_eq!(
        FileCategory::from_path(std::path::Path::new(".rustfmt.toml")),
        FileCategory::Config
    );
}

#[test]
fn file_category_jenkins_is_build() {
    assert_eq!(
        FileCategory::from_path(std::path::Path::new("Jenkinsfile")),
        FileCategory::Build
    );
}

#[test]
fn file_category_containerfile_is_build() {
    assert_eq!(
        FileCategory::from_path(std::path::Path::new("Containerfile")),
        FileCategory::Build
    );
    assert_eq!(
        FileCategory::from_path(std::path::Path::new("podman-compose.yml")),
        FileCategory::Build
    );
    assert_eq!(
        FileCategory::from_path(std::path::Path::new("compose.yaml")),
        FileCategory::Build
    );
}

// ─── Whitespace-only modified symbol detection ──────────────────────────────

#[test]
fn modified_symbol_whitespace_only_detected() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,3 +1,3 @@\n fn foo() {\n-    bar()\n+  bar()\n }",
        1,
        1,
    )]);
    let sym_old = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, false);
    let sym_new = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert!(
        !ctx.symbols_modified.contains("foo"),
        "whitespace-only modified symbol should not appear in symbols_modified: {}",
        ctx.symbols_modified
    );
}

#[test]
fn modified_symbol_semantic_change_shown() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,3 +1,3 @@\n fn foo() {\n-    bar()\n+    baz()\n }",
        1,
        1,
    )]);
    let sym_old = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, false);
    let sym_new = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert!(
        ctx.symbols_modified.contains("foo"),
        "semantic modified symbol should appear in symbols_modified: {}",
        ctx.symbols_modified
    );
}

// ─── Whitespace-only formatting detection ────────────────────────────────────

#[test]
fn all_symbols_whitespace_only_suggests_style() {
    // Diff only changes indentation inside `foo` — no semantic content change.
    // Both old and new symbol share the same name/kind/file → classified as modified.
    // classify_span_change should detect whitespace-only → Style inferred.
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,3 +1,3 @@\n fn foo() {\n-    bar()\n+  bar()\n }",
        1,
        1,
    )]);
    let sym_old = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, false);
    let sym_new = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Style,
        "all whitespace-only modified symbols with no added/removed symbols should suggest Style"
    );
}

#[test]
fn whitespace_detection_works_with_shifted_lines() {
    // Symbol `process` was at line 2 in HEAD, now at line 5 in staged
    // (3 lines added above it). The whitespace change is inside the function.
    // classify_span_change must use separate old/new spans to detect this correctly.
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -2,3 +5,3 @@\n fn process() {\n-    do_thing()\n+  do_thing()\n }",
        1,
        1,
    )]);
    let mut sym_old = make_symbol("process", SymbolKind::Function, "src/lib.rs", true, false);
    sym_old.line = 2;
    sym_old.end_line = 4;
    let mut sym_new = make_symbol("process", SymbolKind::Function, "src/lib.rs", true, true);
    sym_new.line = 5;
    sym_new.end_line = 7;
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    // Should detect as whitespace-only despite line shift
    assert!(
        !ctx.symbols_modified.contains("process"),
        "whitespace-only change with shifted lines should not appear in symbols_modified: {}",
        ctx.symbols_modified
    );
}

// ─── Multi-file diff truncation ──────────────────────────────────────────────

// ─── Locale support ──────────────────────────────────────────────────────────

#[test]
fn locale_instruction_appears_in_prompt_when_set() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-old\n+new",
        1,
        1,
    )]);
    let mut config = default_config();
    config.locale = Some("de".to_string());

    let ctx = ContextBuilder::build(&changes, &[], &[], &config);
    let prompt = ctx.to_prompt();

    assert!(
        prompt.contains("LANGUAGE:"),
        "prompt should contain LANGUAGE instruction when locale is set"
    );
    assert!(
        prompt.contains("Write the subject and body in de"),
        "prompt should instruct writing in the specified language"
    );
    assert!(
        prompt.contains("JSON keys must remain in English"),
        "prompt should instruct keeping JSON keys in English"
    );
}

#[test]
fn no_locale_instruction_when_none() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-old\n+new",
        1,
        1,
    )]);
    let config = default_config();

    let ctx = ContextBuilder::build(&changes, &[], &[], &config);
    let prompt = ctx.to_prompt();

    assert!(
        !prompt.contains("LANGUAGE:"),
        "prompt should not contain LANGUAGE instruction when locale is None"
    );
}

#[test]
fn locale_config_defaults_to_none() {
    let config = Config::default();
    assert!(config.locale.is_none(), "locale should default to None");
}

#[test]
fn locale_deserialized_from_toml() {
    let toml_str = r#"locale = "ja""#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(
        config.locale,
        Some("ja".to_string()),
        "locale should be deserialized from TOML"
    );
}

#[test]
fn locale_absent_in_toml_defaults_to_none() {
    let toml_str = r#"model = "llama3:8b""#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert!(
        config.locale.is_none(),
        "locale should default to None when absent from TOML"
    );
}

// ─── Multi-file diff truncation ──────────────────────────────────────────────

#[test]
fn diff_truncation_multiple_files() {
    let huge_diff = "+line of code\n".repeat(500);
    let files: Vec<_> = (0..10)
        .map(|i| {
            make_file_change(
                &format!("src/module_{}.rs", i),
                ChangeStatus::Modified,
                &huge_diff,
                500,
                0,
            )
        })
        .collect();

    let changes = make_staged_changes(files);

    let mut config = default_config();
    config.max_context_chars = 3_000;

    let ctx = ContextBuilder::build(&changes, &[], &[], &config);
    assert!(
        ctx.truncated_diff.contains("files not shown due to budget")
            || ctx.truncated_diff.contains("budget exceeded")
            || ctx.truncated_diff.contains("lines truncated"),
        "huge multi-file diff should show truncation indicators"
    );
}

// ─── Cross-file symbol connections ───────────────────────────────────────────

#[test]
fn prompt_shows_connections_between_modified_symbols() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/validator.rs",
            ChangeStatus::Modified,
            "+    let result = parse(input);",
            1,
            0,
        ),
        make_file_change(
            "src/services/parser.rs",
            ChangeStatus::Modified,
            "-pub fn parse(s: &str) -> Ast {\n+pub fn parse(s: &str, strict: bool) -> Ast {",
            1,
            1,
        ),
    ]);
    let symbols = vec![
        make_symbol(
            "parse",
            SymbolKind::Function,
            "src/services/parser.rs",
            true,
            true,
        ),
        make_symbol(
            "parse",
            SymbolKind::Function,
            "src/services/parser.rs",
            true,
            false,
        ),
    ];
    let ctx = ContextBuilder::build(&changes, &symbols, &[], &default_config());
    assert!(
        !ctx.connections.is_empty(),
        "should detect that validator.rs calls modified symbol parse()"
    );
}

// ─── Signature display ───────────────────────────────────────────────────────

#[test]
fn prompt_shows_signature_diff_for_modified_symbols() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,3 +1,3 @@\n-pub fn validate(input: &str) -> bool {\n+pub fn validate(input: &str, strict: bool) -> Result<()> {\n     // body\n }",
        1,
        1,
    )]);
    let mut sym_old = make_symbol("validate", SymbolKind::Function, "src/lib.rs", true, false);
    sym_old.signature = Some("pub fn validate(input: &str) -> bool".to_string());
    let mut sym_new = make_symbol("validate", SymbolKind::Function, "src/lib.rs", true, true);
    sym_new.signature =
        Some("pub fn validate(input: &str, strict: bool) -> Result<()>".to_string());
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert!(
        ctx.symbols_modified.contains('\u{2192}') || ctx.symbols_modified.contains("->"),
        "modified symbols should show signature transition: {}",
        ctx.symbols_modified
    );
}

#[test]
fn prompt_shows_signatures_when_available() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+pub fn connect(host: &str) -> Result<()> {\n+    Ok(())\n+}",
        3,
        0,
    )]);
    let mut sym = make_symbol("connect", SymbolKind::Function, "src/lib.rs", true, true);
    sym.signature = Some("pub fn connect(host: &str) -> Result<()>".to_string());
    let ctx = ContextBuilder::build(&changes, &[sym], &[], &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("pub fn connect(host: &str) -> Result<()>"),
        "prompt should contain the full signature, got symbols_added: {}",
        ctx.symbols_added
    );
}

// ─── Test Coverage: detect_primary_change (#35) ───────────────────────────────

#[test]
fn primary_change_prefers_new_public_api() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+pub fn new_api() {}",
        1,
        0,
    )]);
    let sym = make_symbol("new_api", SymbolKind::Function, "src/lib.rs", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym], &[], &default_config());
    assert!(
        ctx.primary_change.as_ref().unwrap().contains("new_api"),
        "should mention new public API: {:?}",
        ctx.primary_change
    );
}

#[test]
fn primary_change_falls_back_to_removed_public() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-pub fn old_api() {}",
        0,
        1,
    )]);
    let sym = make_symbol("old_api", SymbolKind::Function, "src/lib.rs", true, false);
    let ctx = ContextBuilder::build(&changes, &[sym], &[], &default_config());
    assert!(
        ctx.primary_change.as_ref().unwrap().contains("old_api"),
        "should mention removed public API: {:?}",
        ctx.primary_change
    );
}

#[test]
fn primary_change_falls_back_to_largest_file() {
    let changes = make_staged_changes(vec![
        make_file_change("src/a.rs", ChangeStatus::Modified, "+x", 1, 0),
        make_file_change("src/b.rs", ChangeStatus::Modified, "+large change", 50, 10),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.primary_change.as_ref().unwrap().contains("b"),
        "should mention largest file: {:?}",
        ctx.primary_change
    );
}

// ─── Test Coverage: detect_metadata_breaking (#36) ────────────────────────────

#[test]
fn metadata_breaking_detects_msrv_change() {
    let changes = make_staged_changes(vec![make_file_change(
        "Cargo.toml",
        ChangeStatus::Modified,
        "+rust-version = \"1.75\"",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        !ctx.metadata_breaking_signals.is_empty(),
        "should detect MSRV change"
    );
}

#[test]
fn metadata_breaking_detects_pub_use_removal() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "-pub use crate::old_api::*;",
        0,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.metadata_breaking_signals
            .iter()
            .any(|s| s.contains("pub use")),
        "should detect removed pub use: {:?}",
        ctx.metadata_breaking_signals
    );
}

// ─── Test Coverage: detect_bug_evidence all patterns (#38) ────────────────────

#[test]
fn bug_evidence_detects_hash_fix() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.py",
        ChangeStatus::Modified,
        "+# fix: off by one",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(ctx.has_bug_evidence, "should detect '# fix' pattern");
}

#[test]
fn bug_evidence_detects_c_style_fix() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.c",
        ChangeStatus::Modified,
        "+/* fix: memory leak */",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(ctx.has_bug_evidence, "should detect '/* fix' pattern");
}

#[test]
fn bug_evidence_detects_bug_keyword() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+// bug: incorrect index",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(ctx.has_bug_evidence, "should detect '// bug' pattern");
}

#[test]
fn bug_evidence_detects_fixme() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+// FIXME this is broken",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(ctx.has_bug_evidence, "should detect 'fixme' pattern");
}

#[test]
fn bug_evidence_detects_hotfix() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+// hotfix for prod issue",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(ctx.has_bug_evidence, "should detect 'hotfix' pattern");
}

// ─── Test Coverage: connection content assertion (#41) ────────────────────────

#[test]
fn connection_content_mentions_symbol_name() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/validator.rs",
            ChangeStatus::Modified,
            "+    let result = parse(input);",
            1,
            0,
        ),
        make_file_change(
            "src/services/parser.rs",
            ChangeStatus::Modified,
            "-pub fn parse(s: &str) -> Ast {\n+pub fn parse(s: &str, strict: bool) -> Ast {",
            1,
            1,
        ),
    ]);
    let symbols = vec![
        make_symbol(
            "parse",
            SymbolKind::Function,
            "src/services/parser.rs",
            true,
            true,
        ),
        make_symbol(
            "parse",
            SymbolKind::Function,
            "src/services/parser.rs",
            true,
            false,
        ),
    ];
    let ctx = ContextBuilder::build(&changes, &symbols, &[], &default_config());
    assert!(
        ctx.connections.iter().any(|c| c.contains("parse")),
        "connection should mention symbol name 'parse': {:?}",
        ctx.connections
    );
}

// ─── Test Coverage: Deleted/Renamed status (#37) ──────────────────────────────

#[test]
fn format_files_shows_deleted_marker() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/old.rs",
        ChangeStatus::Deleted,
        "-pub fn removed() {}",
        0,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.file_breakdown.contains("[-]"),
        "deleted file should show [-] marker: {}",
        ctx.file_breakdown
    );
}

#[test]
fn format_files_shows_renamed_marker() {
    let changes = make_staged_changes(vec![make_renamed_file(
        "src/old_name.rs",
        "src/new_name.rs",
        95,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.file_breakdown.contains("[R]"),
        "renamed file should show [R] marker: {}",
        ctx.file_breakdown
    );
    assert!(
        ctx.file_breakdown.contains("95% similar"),
        "renamed file should show similarity: {}",
        ctx.file_breakdown
    );
}

// ─── Test Coverage: classify_span_change None path (#39) ──────────────────────

#[test]
fn whitespace_detection_returns_none_when_span_has_no_changes() {
    // Symbol at lines 50-60 but diff hunk is at lines 1-3 — no overlap
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,3 +1,3 @@\n fn other() {\n-    old()\n+    new()\n }",
        1,
        1,
    )]);
    let mut sym_old = make_symbol("distant", SymbolKind::Function, "src/lib.rs", true, false);
    sym_old.line = 50;
    sym_old.end_line = 60;
    let mut sym_new = make_symbol("distant", SymbolKind::Function, "src/lib.rs", true, true);
    sym_new.line = 50;
    sym_new.end_line = 60;
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    // Symbol is outside the hunk — classify_span_change returns None (no changes in span).
    // The symbol still appears as "modified" (name+kind+file match) but with no
    // whitespace classification. This is expected: it won't be filtered as whitespace-only.
    // This test verifies the None path doesn't crash or produce false positives.
    assert!(
        ctx.symbols_modified.contains("distant"),
        "modified symbol outside hunk should still appear (with no ws classification): {}",
        ctx.symbols_modified
    );
}

// Direct assertions on the `None` branch of `classify_span_change` via the
// public `classify_diff_span` wrapper re-exported from `lib.rs`. The function
// returns `None` exactly when no added/removed lines fall inside the requested
// (`new_start..=new_end`) / (`old_start..=old_end`) spans — see the
// `added_in_span.is_empty() && removed_in_span.is_empty()` guard in
// `src/services/context.rs`. These tests pin that contract so a future
// refactor cannot silently convert the "no changes in span" case into a
// false-positive `Some(true)` whitespace-only classification.

#[test]
fn classify_span_change_returns_none_when_span_is_outside_hunk() {
    // Hunk touches lines 1-3 in both old and new files, but we query a
    // symbol span at lines 50-60. No +/- line lands inside the span, so
    // the function must short-circuit to `None` before the whitespace
    // comparison runs.
    let diff = "@@ -1,3 +1,3 @@\n fn other() {\n-    old()\n+    new()\n }\n";
    assert_eq!(
        classify_diff_span(diff, 50, 60, 50, 60),
        None,
        "span entirely outside the hunk must yield None"
    );
}

#[test]
fn classify_span_change_returns_none_for_empty_diff() {
    // Empty diff: the parse loop never enters a hunk, so both
    // `added_in_span` and `removed_in_span` stay empty and the function
    // hits the early-return `None`.
    assert_eq!(
        classify_diff_span("", 1, 10, 1, 10),
        None,
        "empty diff must yield None"
    );
}

#[test]
fn classify_span_change_returns_none_when_span_range_is_empty() {
    // Span with `new_start > new_end` (and likewise for `old`) never
    // matches any +/- line because `in_new_span`/`in_old_span` evaluate
    // `false` for every counter value. This is a degenerate but reachable
    // input from callers that derive span bounds from AST nodes whose
    // `end < start` (e.g. zero-length nodes), so the `None` short-circuit
    // must hold here too.
    let diff = "@@ -1,3 +1,3 @@\n fn f() {\n-    old()\n+    new()\n }\n";
    assert_eq!(
        classify_diff_span(diff, 100, 50, 100, 50),
        None,
        "inverted span (start > end) must yield None"
    );
}

// ─── Import change detection ─────────────────────────────────────────────────

#[test]
fn detect_rust_import_changes() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/analyzer.rs",
        ChangeStatus::Modified,
        "+use crate::domain::DiffHunk;\n-use crate::old_module::OldType;\n context line",
        1,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(ctx.import_changes.len(), 2);
    assert!(ctx.import_changes[0].contains("added"));
    assert!(ctx.import_changes[0].contains("use crate::domain::DiffHunk"));
    assert!(ctx.import_changes[1].contains("removed"));
}

#[test]
fn detect_python_import_changes() {
    let changes = make_staged_changes(vec![make_file_change(
        "app/main.py",
        ChangeStatus::Modified,
        "+from flask import Blueprint\n+import os\n",
        2,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(ctx.import_changes.len(), 2);
}

#[test]
fn detect_cpp_include_changes() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/main.cpp",
        ChangeStatus::Modified,
        "+#include <vector>\n-#include <list>\n",
        1,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(ctx.import_changes.len(), 2);
    assert!(ctx.import_changes[0].contains("#include <vector>"));
}

#[test]
fn import_changes_capped_at_10() {
    let diff: String = (0..15)
        .map(|i| format!("+use crate::mod_{i}::Type;\n"))
        .collect();
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        &diff,
        15,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(ctx.import_changes.len(), 10);
}

#[test]
fn import_changes_shown_in_prompt() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+use crate::new_dep::Thing;\n",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("IMPORTS CHANGED:"),
        "prompt should contain imports section"
    );
    assert!(prompt.contains("use crate::new_dep::Thing"));
}

// ─── Test Coverage: HARD LIMIT dedup check (#28) ─────────────────────────────

#[test]
fn prompt_hard_limit_includes_char_budget() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+fn foo() {}",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("HARD LIMIT"),
        "prompt should contain HARD LIMIT section"
    );
    assert!(
        prompt.contains("chars"),
        "HARD LIMIT should mention char budget"
    );
}

// ─── Test-to-code ratio inference ────────────────────────────────────────────

#[test]
fn mostly_test_additions_suggests_test_type() {
    // 90 test additions + 10 source additions → test
    let changes = make_staged_changes(vec![
        make_file_change(
            "tests/foo.rs",
            ChangeStatus::Modified,
            &"+test line\n".repeat(9),
            90,
            0,
        ),
        make_file_change("src/lib.rs", ChangeStatus::Modified, "+code\n", 10, 0),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(ctx.suggested_type, CommitType::Test);
}

#[test]
fn balanced_test_and_source_does_not_suggest_test() {
    // 50/50 split → should NOT be test
    let changes = make_staged_changes(vec![
        make_file_change("tests/foo.rs", ChangeStatus::Modified, "+test\n", 50, 0),
        make_file_change("src/lib.rs", ChangeStatus::Modified, "+code\n", 50, 0),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_ne!(ctx.suggested_type, CommitType::Test);
}

// ─── Doc-vs-code change distinction (SpanChangeKind) ─────────────────────────

#[test]
fn doc_only_change_classified_as_docs() {
    // Diff changes only a doc comment inside function `foo` (lines 1-4)
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,4 +1,4 @@\n fn foo() {\n-    /// old doc\n+    /// new doc\n }",
        1,
        1,
    )]);
    let sym_old = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, false);
    let sym_new = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Docs,
        "all-doc-only modified symbols with no added/removed symbols should suggest Docs"
    );
}

#[test]
fn mixed_doc_and_code_change_not_docs_type() {
    // Diff changes both a doc comment and a code line inside function `foo`
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,5 +1,5 @@\n fn foo() {\n-    /// old doc\n+    /// new doc\n-    let x = 1;\n+    let x = 2;\n }",
        2,
        2,
    )]);
    let sym_old = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, false);
    let sym_new = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert_ne!(
        ctx.suggested_type,
        CommitType::Docs,
        "mixed doc + code change should not suggest Docs"
    );
}

#[test]
fn doc_only_modified_symbol_shows_docs_suffix() {
    // Diff changes only a doc comment — the symbol should appear in symbols_modified
    // with a [docs only] suffix (it's not whitespace-only, so it passes the filter)
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,4 +1,4 @@\n fn foo() {\n-    /// old doc\n+    /// new doc\n }",
        1,
        1,
    )]);
    let sym_old = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, false);
    let sym_new = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert!(
        ctx.symbols_modified.contains("[docs only]"),
        "doc-only modified symbol should show [docs only] suffix: {}",
        ctx.symbols_modified
    );
}

#[test]
fn mixed_doc_code_modified_symbol_shows_mixed_suffix() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,5 +1,5 @@\n fn foo() {\n-    /// old doc\n+    /// new doc\n-    let x = 1;\n+    let x = 2;\n }",
        2,
        2,
    )]);
    let sym_old = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, false);
    let sym_new = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert!(
        ctx.symbols_modified.contains("[docs + code]"),
        "mixed doc+code modified symbol should show [docs + code] suffix: {}",
        ctx.symbols_modified
    );
}

#[test]
fn semantic_only_change_has_no_suffix() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "@@ -1,3 +1,3 @@\n fn foo() {\n-    bar()\n+    baz()\n }",
        1,
        1,
    )]);
    let sym_old = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, false);
    let sym_new = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert!(
        !ctx.symbols_modified.contains("[docs"),
        "purely semantic change should have no doc suffix: {}",
        ctx.symbols_modified
    );
}

// ─── Test file correlation detection ──────────────────────────────────────────

#[test]
fn detect_test_file_correlation() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/context.rs",
            ChangeStatus::Modified,
            "+code\n",
            1,
            0,
        ),
        make_file_change("tests/context.rs", ChangeStatus::Modified, "+test\n", 1, 0),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert_eq!(ctx.test_correlations.len(), 1);
    assert!(ctx.test_correlations[0].contains("context"));
}

#[test]
fn no_correlation_without_matching_test() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/context.rs",
            ChangeStatus::Modified,
            "+code\n",
            1,
            0,
        ),
        make_file_change("tests/other.rs", ChangeStatus::Modified, "+test\n", 1, 0),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(ctx.test_correlations.is_empty());
}

#[test]
fn test_correlation_shown_in_prompt() {
    let changes = make_staged_changes(vec![
        make_file_change(
            "src/services/context.rs",
            ChangeStatus::Modified,
            "+code\n",
            1,
            0,
        ),
        make_file_change("tests/context.rs", ChangeStatus::Modified, "+test\n", 1, 0),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();
    assert!(prompt.contains("RELATED FILES:"));
}

#[test]
fn test_correlations_capped_at_5() {
    let mut files = Vec::new();
    for i in 0..8 {
        files.push(make_file_change(
            &format!("src/mod{i}.rs"),
            ChangeStatus::Modified,
            "+code\n",
            1,
            0,
        ));
        files.push(make_file_change(
            &format!("tests/mod{i}.rs"),
            ChangeStatus::Modified,
            "+test\n",
            1,
            0,
        ));
    }
    let changes = make_staged_changes(files);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(ctx.test_correlations.len() <= 5);
}

// ─── Structured changes in prompt ─────────────────────────────────────────────

#[test]
fn structured_changes_shown_in_prompt() {
    use commitbee::domain::diff::{ChangeDetail, SymbolDiff};

    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+pub fn validate(input: &str, strict: bool) -> Result<()> { Ok(()) }",
        1,
        0,
    )]);
    let diffs = vec![SymbolDiff {
        name: "validate".into(),
        file: "src/lib.rs".into(),
        line: 1,
        parent_scope: Some("Validator".into()),
        changes: vec![
            ChangeDetail::ParamAdded("strict: bool".into()),
            ChangeDetail::ReturnTypeChanged {
                old: "bool".into(),
                new: "Result<()>".into(),
            },
        ],
    }];
    let ctx = ContextBuilder::build(&changes, &[], &diffs, &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("STRUCTURED CHANGES:"),
        "prompt should contain structured changes section"
    );
    assert!(
        prompt.contains("Validator::validate()"),
        "should show parent scope"
    );
    assert!(
        prompt.contains("+param strict: bool"),
        "should show added param"
    );
    assert!(
        prompt.contains("return bool"),
        "should show return type change"
    );
}

#[test]
fn empty_structured_changes_not_shown() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+code\n",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        !prompt.contains("STRUCTURED CHANGES:"),
        "empty structured changes should not appear"
    );
}

#[test]
fn python_comment_only_change_classified_as_docs() {
    let changes = make_staged_changes(vec![make_file_change(
        "app/main.py",
        ChangeStatus::Modified,
        "@@ -1,3 +1,3 @@\n def process():\n-    # old comment\n+    # new comment\n     pass",
        1,
        1,
    )]);
    let sym_old = make_symbol("process", SymbolKind::Function, "app/main.py", true, false);
    let sym_new = make_symbol("process", SymbolKind::Function, "app/main.py", true, true);
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &[], &default_config());
    assert_eq!(
        ctx.suggested_type,
        CommitType::Docs,
        "Python comment-only change should suggest Docs"
    );
}

// ─── Token budget rebalancing with structural diffs ───────────────────────────

#[test]
fn symbol_budget_reduced_when_structural_diffs_present() {
    use commitbee::domain::diff::{ChangeDetail, SymbolDiff};

    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        &"+pub fn foo() {}\n".repeat(50),
        50,
        0,
    )]);
    let mut sym = make_symbol("foo", SymbolKind::Function, "src/lib.rs", true, true);
    sym.signature = Some("pub fn foo()".into());
    let diffs = vec![SymbolDiff {
        name: "foo".into(),
        file: "src/lib.rs".into(),
        line: 1,
        parent_scope: None,
        changes: vec![ChangeDetail::BodyModified {
            additions: 5,
            deletions: 2,
        }],
    }];

    // With diffs: symbol budget should be reduced (20%)
    let ctx_with = ContextBuilder::build(&changes, &[sym.clone()], &diffs, &default_config());
    // Without diffs: symbol budget at 30% (signatures present)
    let ctx_without = ContextBuilder::build(&changes, &[sym], &[], &default_config());

    // The diff section should be larger when structural diffs reduce symbol budget
    assert!(
        ctx_with.truncated_diff.len() >= ctx_without.truncated_diff.len(),
        "structural diffs should free budget for raw diff: with={} without={}",
        ctx_with.truncated_diff.len(),
        ctx_without.truncated_diff.len()
    );
}

// ─── Unsafe addition constraint ───────────────────────────────────────────────

#[test]
fn unsafe_addition_triggers_constraint() {
    use commitbee::domain::diff::{ChangeDetail, SymbolDiff};

    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+unsafe fn process() {}\n",
        1,
        0,
    )]);
    let diffs = vec![SymbolDiff {
        name: "process".into(),
        file: "src/lib.rs".into(),
        line: 1,
        parent_scope: None,
        changes: vec![ChangeDetail::UnsafeAdded],
    }];
    let ctx = ContextBuilder::build(&changes, &[], &diffs, &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("Unsafe code added"),
        "should contain unsafe constraint: {}",
        prompt
    );
}

// ─── Change intent detection ──────────────────────────────────────────────────

#[test]
fn detect_error_handling_intent() {
    let diff = "+    let result = validate(input)?;\n\
                +    let data = parse(raw).map_err(|e| Error::Parse(e))?;\n\
                +    if let Err(e) = process() {\n\
                +        return Err(e);\n\
                +    }\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        diff,
        4,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        !ctx.intents.is_empty(),
        "should detect error handling intent"
    );
    assert_eq!(ctx.intents[0].kind, IntentKind::ErrorHandlingAdded);
}

#[test]
fn detect_test_added_intent() {
    let diff = "+#[test]\n\
                +fn test_validation() {\n\
                +    assert_eq!(validate(\"ok\"), true);\n\
                +    assert!(check());\n\
                +}\n";
    let changes = make_staged_changes(vec![make_file_change(
        "tests/validation.rs",
        ChangeStatus::Added,
        diff,
        5,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.intents.iter().any(|i| i.kind == IntentKind::TestAdded),
        "should detect test intent: {:?}",
        ctx.intents
    );
}

#[test]
fn detect_dependency_update_intent() {
    let diff = "+tokio = \"1.40\"\n\
                -tokio = \"1.38\"\n\
                +serde = { version = \"1.0.210\" }\n";
    let changes = make_staged_changes(vec![make_file_change(
        "Cargo.toml",
        ChangeStatus::Modified,
        diff,
        2,
        1,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.intents
            .iter()
            .any(|i| i.kind == IntentKind::DependencyUpdate),
        "should detect dependency update: {:?}",
        ctx.intents
    );
}

#[test]
fn intent_shown_in_prompt() {
    let diff = "+    let result = validate(input)?;\n\
                +    let data = parse(raw).map_err(|e| Error::Parse(e))?;\n\
                +    if let Err(e) = process() {\n\
                +        return Err(e);\n\
                +    }\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        diff,
        4,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("INTENT:"),
        "prompt should show intent section"
    );
}

#[test]
fn no_intent_for_small_changes() {
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        "+let x = 1;\n",
        1,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.intents.is_empty(),
        "small change should not trigger intent"
    );
}

#[test]
fn detect_logging_intent() {
    let diff = "+    debug!(\"entering function\");\n\
                +    info!(\"processing {} items\", count);\n\
                +    warn!(\"deprecated API called\");\n\
                +    error!(\"failed to connect\");\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        diff,
        4,
        0,
    )]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.intents
            .iter()
            .any(|i| i.kind == IntentKind::LoggingAdded),
        "should detect logging intent: {:?}",
        ctx.intents
    );
}

#[test]
fn intents_capped_at_three() {
    // Build a diff that triggers error handling, test, and logging patterns plus dep update
    let diff = "+    let result = validate(input)?;\n\
                +    let data = parse(raw).map_err(|e| Error::Parse(e))?;\n\
                +    if let Err(e) = process() {\n\
                +#[test]\n\
                +fn test_foo() {\n\
                +    assert!(true);\n\
                +    assert_eq!(1, 1);\n\
                +    debug!(\"test\");\n\
                +    info!(\"test\");\n\
                +    warn!(\"test\");\n\
                +    error!(\"test\");\n";
    let changes = make_staged_changes(vec![
        make_file_change("src/lib.rs", ChangeStatus::Modified, diff, 11, 0),
        make_file_change(
            "Cargo.toml",
            ChangeStatus::Modified,
            "+tokio = { version = \"1.40\" }\n",
            1,
            0,
        ),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &[], &default_config());
    assert!(
        ctx.intents.len() <= 3,
        "intents should be capped at 3, got {}",
        ctx.intents.len()
    );
}
