// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

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
        is_public,
        is_added,
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
fn infer_type_small_change_is_fix() {
    // <20 insertions and <20 deletions, no special symbols or categories
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
        CommitType::Fix,
        "small change (<20 insertions and deletions) should infer Fix"
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
fn infer_type_default_fallback_is_feat() {
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
        CommitType::Feat,
        "large non-special change should fallback to Feat"
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
