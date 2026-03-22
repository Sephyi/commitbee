// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod helpers;

use std::path::PathBuf;

use commitbee::config::Config;
use commitbee::domain::{ChangeStatus, CodeSymbol, CommitType, FileCategory, SymbolKind};
use commitbee::services::context::ContextBuilder;
use helpers::{make_file_change, make_staged_changes};

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
        signature: None,
    }
}

// ─── CommitType inference ─────────────────────────────────────────────────────

#[test]
fn infer_type_all_docs() {
    let changes = make_staged_changes(vec![
        make_file_change("README.md", ChangeStatus::Modified, "", 5, 2),
        make_file_change("CHANGELOG.md", ChangeStatus::Modified, "", 3, 1),
    ]);
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &symbols, &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &symbols, &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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

    let ctx = ContextBuilder::build(&changes, &[], &config);
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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

    let ctx = ContextBuilder::build(&changes, &symbols, &config);
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

    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &symbols, &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &symbols, &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let commit_type = ContextBuilder::infer_commit_type(&changes, &symbols);
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
    let commit_type = ContextBuilder::infer_commit_type(&changes, &symbols);
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &symbols, &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &default_config());
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
    let ctx = ContextBuilder::build(&changes, &[sym_old, sym_new], &default_config());
    assert!(
        ctx.symbols_modified.contains("foo"),
        "semantic modified symbol should appear in symbols_modified: {}",
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

    let ctx = ContextBuilder::build(&changes, &[], &config);
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

    let ctx = ContextBuilder::build(&changes, &[], &config);
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

    let ctx = ContextBuilder::build(&changes, &[], &config);
    assert!(
        ctx.truncated_diff.contains("files not shown due to budget")
            || ctx.truncated_diff.contains("budget exceeded")
            || ctx.truncated_diff.contains("lines truncated"),
        "huge multi-file diff should show truncation indicators"
    );
}

// ─── Signature display ───────────────────────────────────────────────────────

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
    let ctx = ContextBuilder::build(&changes, &[sym], &default_config());
    let prompt = ctx.to_prompt();
    assert!(
        prompt.contains("pub fn connect(host: &str) -> Result<()>"),
        "prompt should contain the full signature, got symbols_added: {}",
        ctx.symbols_added
    );
}
