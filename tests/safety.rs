// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod helpers;

use commitbee::domain::ChangeStatus;
use commitbee::services::safety::{
    check_for_conflicts, scan_for_secrets, scan_full_diff_for_secrets,
};
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

// ─── Additional secret patterns ──────────────────────────────────────────────

#[test]
fn detects_anthropic_key() {
    // sk-ant- followed by exactly 80 alphanumeric characters
    let key = format!("+sk-ant-{}\n", "a".repeat(80));
    let changes = make_staged_changes(vec![make_file_change(
        "src/llm.rs",
        ChangeStatus::Modified,
        &key,
        1,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(
        !matches.is_empty(),
        "expected at least one secret match for Anthropic key"
    );
    assert_eq!(matches[0].pattern_name, "Anthropic Key");
}

#[test]
fn multiple_secrets_same_file() {
    let diff = "+API_KEY=abcdefghijklmnopqrstuvwxyz1234567890abcdef\n\
         +AKIAIOSFODNN7EXAMPLE\n\
         +password = \"super_secret_value\"\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/config.rs",
        ChangeStatus::Modified,
        diff,
        3,
        0,
    )]);

    let matches = scan_for_secrets(&changes);
    assert!(
        matches.len() >= 3,
        "expected at least 3 secret matches from 3 different lines, got {}",
        matches.len()
    );

    let names: Vec<&str> = matches.iter().map(|m| m.pattern_name.as_str()).collect();
    assert!(names.contains(&"API Key"), "should detect API Key");
    assert!(names.contains(&"AWS Key"), "should detect AWS Key");
    assert!(
        names.contains(&"Generic Secret"),
        "should detect Generic Secret"
    );
}

// ─── Additional conflict detection ───────────────────────────────────────────

#[test]
fn ignores_conflict_markers_in_doc_paths() {
    let diff = "\
+<<<<<<< HEAD\n\
+this is a documentation example of merge conflicts\n\
+>>>>>>> main\n\
";
    let changes = make_staged_changes(vec![make_file_change(
        "docs/merge-guide.md",
        ChangeStatus::Modified,
        diff,
        3,
        0,
    )]);

    assert!(
        !check_for_conflicts(&changes),
        "conflict markers in docs/ paths should not be reported"
    );
}

// ─── Full untruncated diff scanner ───────────────────────────────────────────

#[test]
fn full_diff_detects_secret_in_added_lines() {
    let full_diff = "\
diff --git a/src/config.rs b/src/config.rs
--- a/src/config.rs
+++ b/src/config.rs
@@ -1,3 +1,4 @@
 use std::env;
+API_KEY=abcdefghijklmnopqrstuvwxyz1234567890abcdef
 fn main() {}
";
    let matches = scan_full_diff_for_secrets(full_diff);
    assert!(!matches.is_empty(), "expected secret in full diff");
    assert_eq!(matches[0].pattern_name, "API Key");
    assert_eq!(matches[0].file, "src/config.rs");
}

#[test]
fn full_diff_ignores_removed_lines() {
    let full_diff = "\
diff --git a/src/config.rs b/src/config.rs
--- a/src/config.rs
+++ b/src/config.rs
@@ -1,4 +1,3 @@
 use std::env;
-API_KEY=abcdefghijklmnopqrstuvwxyz1234567890abcdef
 fn main() {}
";
    let matches = scan_full_diff_for_secrets(full_diff);
    assert!(
        matches.is_empty(),
        "removed lines should not trigger secrets"
    );
}

#[test]
fn full_diff_catches_secret_beyond_truncation() {
    // Simulate a long diff where the secret is well past what max_file_lines would truncate
    let mut diff = String::from(
        "diff --git a/src/big.rs b/src/big.rs\n\
         --- a/src/big.rs\n\
         +++ b/src/big.rs\n\
         @@ -1,200 +1,201 @@\n",
    );
    // 150 normal added lines (past typical max_file_lines=100)
    for i in 0..150 {
        diff.push_str(&format!("+let x{} = {};\n", i, i));
    }
    // Secret on line 151
    diff.push_str("+AKIAIOSFODNN7EXAMPLE\n");

    let matches = scan_full_diff_for_secrets(&diff);
    assert!(
        !matches.is_empty(),
        "secret after truncation point should be caught"
    );
    assert_eq!(matches[0].pattern_name, "AWS Key");
    assert_eq!(matches[0].file, "src/big.rs");
}

#[test]
fn full_diff_tracks_multiple_files() {
    let full_diff = "\
diff --git a/src/a.rs b/src/a.rs
--- a/src/a.rs
+++ b/src/a.rs
@@ -1,2 +1,3 @@
 fn a() {}
+let normal = true;
diff --git a/src/b.rs b/src/b.rs
--- a/src/b.rs
+++ b/src/b.rs
@@ -1,2 +1,3 @@
 fn b() {}
+sk-abcdefghijklmnopqrstuvwxyz1234567890abcdefghijkl
";
    let matches = scan_full_diff_for_secrets(full_diff);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].file, "src/b.rs");
    assert_eq!(matches[0].pattern_name, "OpenAI Key");
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
    fn full_diff_scanner_never_panics(input in proptest::prelude::any::<String>()) {
        // Must not panic regardless of input
        let _ = scan_full_diff_for_secrets(&input);
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
