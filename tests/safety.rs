// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

mod helpers;

use commitbee::domain::ChangeStatus;
use commitbee::services::safety::{check_for_conflicts, scan_for_secrets};
use helpers::{make_file_change, make_staged_changes};

// ─── Secret detection: one test per pattern ───────────────────────────────────

#[test]
fn detects_api_key_pattern() {
    let diff = "+API_KEY=abcdefghijklmnopqrstuvwxyz1234567890abcdef\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/config.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "expected at least one secret match");
    assert_eq!(matches[0].pattern_name, "API Key");
}

#[test]
fn detects_aws_key() {
    let diff = "+AKIAIOSFODNN7EXAMPLE\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/aws.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "expected at least one secret match");
    assert_eq!(matches[0].pattern_name, "AWS Key");
}

#[test]
fn detects_openai_key() {
    // sk- followed by exactly 48 alphanumeric characters
    let diff = "+sk-abcdefghijklmnopqrstuvwxyz1234567890abcdefghijkl\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/llm.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "expected at least one secret match");
    assert_eq!(matches[0].pattern_name, "OpenAI Key");
}

#[test]
fn detects_private_key() {
    let diff = "+-----BEGIN RSA PRIVATE KEY-----\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/crypto.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "expected at least one secret match");
    assert_eq!(matches[0].pattern_name, "Private Key");
}

#[test]
fn detects_connection_string() {
    let diff = "+DATABASE_URL=postgres://user:pass@host:5432/db\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/db.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "expected at least one secret match");
    assert_eq!(matches[0].pattern_name, "Connection String");
}

#[test]
fn detects_generic_secret() {
    let diff = "+password = \"super_secret_value\"\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/auth.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "expected at least one secret match");
    assert_eq!(matches[0].pattern_name, "Generic Secret");
}

// ─── False positive prevention ────────────────────────────────────────────────

#[test]
fn no_false_positive_on_normal_code() {
    let diff = "\
+// Configure the API client\n\
+let config = Config::new();\n\
+let key = config.get_key();\n\
+fn process_token(input: &str) -> String {\n\
";
    let changes = make_staged_changes(vec![make_file_change(
        "src/client.rs",
        ChangeStatus::Modified,
        diff,
        4,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(
        matches.is_empty(),
        "expected no matches for normal code, got: {:?}",
        matches.iter().map(|m| &m.pattern_name).collect::<Vec<_>>()
    );
}

#[test]
fn ignores_deleted_lines() {
    // A deleted line starting with '-' should not trigger secret detection
    let diff = "-API_KEY=abcdefghijklmnopqrstuvwxyz1234567890abcdef\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/config.rs",
        ChangeStatus::Modified,
        diff,
        0,
        1,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(
        matches.is_empty(),
        "deleted lines should not be scanned for secrets"
    );
}

#[test]
fn ignores_diff_headers() {
    // Lines starting with '+++' are diff headers, not added content
    let diff = "+++ b/src/config.rs\n+some normal code here\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/config.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(
        matches.is_empty(),
        "diff header lines (starting with +++) should be ignored"
    );
}

#[test]
fn skips_binary_files() {
    let diff = "+API_KEY=abcdefghijklmnopqrstuvwxyz1234567890abcdef\n";
    let mut file_change = make_file_change("assets/image.png", ChangeStatus::Modified, diff, 1, 0);
    file_change.is_binary = true;

    let changes = make_staged_changes(vec![file_change]);
    let matches = scan_for_secrets(&changes);
    assert!(
        matches.is_empty(),
        "binary files should be skipped during secret scanning"
    );
}

// ─── Conflict detection ───────────────────────────────────────────────────────

#[test]
fn detects_conflict_markers() {
    let diff = "\
 fn foo() {\n\
+<<<<<<< HEAD\n\
+    let x = 1;\n\
+=======\n\
+    let x = 2;\n\
+>>>>>>> feature-branch\n\
 }\n\
";
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        diff,
        5,
        0,
    )]);

    assert!(
        check_for_conflicts(&changes),
        "conflict markers in source file should be detected"
    );
}

#[test]
fn ignores_conflict_markers_in_tests() {
    // Paths containing "test" are intentionally skipped by check_for_conflicts
    let diff = "\
+<<<<<<< HEAD\n\
+this is a test fixture for merge conflicts\n\
+>>>>>>> main\n\
";
    let changes = make_staged_changes(vec![make_file_change(
        "tests/fixtures/merge.txt",
        ChangeStatus::Modified,
        diff,
        3,
        0,
    )]);

    assert!(
        !check_for_conflicts(&changes),
        "conflict markers in test fixtures should not be reported"
    );
}

#[test]
fn no_conflicts_in_clean_diff() {
    let diff = "\
-fn old_function() {\n\
-    println!(\"old\");\n\
+fn new_function() {\n\
+    println!(\"new\");\n\
 }\n\
";
    let changes = make_staged_changes(vec![make_file_change(
        "src/lib.rs",
        ChangeStatus::Modified,
        diff,
        2,
        2,
    )]);

    assert!(
        !check_for_conflicts(&changes),
        "clean diff should not report conflict markers"
    );
}

// ─── Proptest: never-panic guarantees ─────────────────────────────────────────

proptest::proptest! {
    #[test]
    fn secret_scanner_never_panics(input in proptest::prelude::any::<String>()) {
        let changes = make_staged_changes(vec![make_file_change(
            "src/fuzz.rs",
            ChangeStatus::Modified,
            &input,
            0,
            0,
        )]);
        // Must not panic regardless of input
        let _ = scan_for_secrets(&changes);
    }

    #[test]
    fn conflict_checker_never_panics(input in proptest::prelude::any::<String>()) {
        let changes = make_staged_changes(vec![make_file_change(
            "src/fuzz.rs",
            ChangeStatus::Modified,
            &input,
            0,
            0,
        )]);
        // Must not panic regardless of input
        let _ = check_for_conflicts(&changes);
    }
}
