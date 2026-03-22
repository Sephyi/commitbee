// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

mod helpers;

use commitbee::domain::ChangeStatus;
use commitbee::services::safety::{
    build_patterns, check_for_conflicts, scan_for_secrets, scan_for_secrets_with_patterns,
    scan_full_diff_for_secrets, scan_full_diff_with_patterns,
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
    assert_eq!(matches[0].pattern_name, "Generic API Key");
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
    assert_eq!(matches[0].pattern_name, "AWS Access Key");
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
    assert!(
        names.contains(&"Generic API Key"),
        "should detect Generic API Key"
    );
    assert!(
        names.contains(&"AWS Access Key"),
        "should detect AWS Access Key"
    );
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
    assert_eq!(matches[0].pattern_name, "Generic API Key");
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
    assert_eq!(matches[0].pattern_name, "AWS Access Key");
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

// ─── New pattern detection tests ──────────────────────────────────────────────

#[test]
fn detects_github_fine_grained_token() {
    let diff = "+github_pat_abcdefghijklmnopqrstuvwx\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/ci.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);
    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "should detect GitHub fine-grained PAT");
    assert_eq!(matches[0].pattern_name, "GitHub Fine-Grained Token");
}

#[test]
fn detects_gitlab_token() {
    let diff = "+glpat-abcdefghijklmnopqrstuvwxyz\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/ci.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);
    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "should detect GitLab PAT");
    assert_eq!(matches[0].pattern_name, "GitLab Token");
}

#[test]
fn detects_slack_token() {
    let diff = "+xoxb-1234567890-abcdefghijklmnop\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/notify.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);
    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "should detect Slack token");
    assert_eq!(matches[0].pattern_name, "Slack Token");
}

#[test]
fn detects_stripe_key() {
    let diff = "+sk_live_abcdefghijklmnopqrstuvwxyz\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/billing.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);
    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "should detect Stripe key");
    assert_eq!(matches[0].pattern_name, "Stripe Key");
}

#[test]
fn detects_gcp_api_key() {
    let diff = "+AIzaSyA1234567890abcdefghijklmnopqrstuv\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/gcp.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);
    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "should detect GCP API key");
    assert_eq!(matches[0].pattern_name, "GCP API Key");
}

#[test]
fn detects_huggingface_token() {
    let diff = "+hf_abcdefghijklmnopqrstuvwxyz12345678\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/ml.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);
    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "should detect HuggingFace token");
    assert_eq!(matches[0].pattern_name, "HuggingFace Token");
}

#[test]
fn detects_sendgrid_key() {
    let key = format!("+SG.{}.{}\n", "a".repeat(22), "b".repeat(43));
    let changes = make_staged_changes(vec![make_file_change(
        "src/email.rs",
        ChangeStatus::Modified,
        &key,
        1,
        0,
    )]);
    let matches = scan_for_secrets(&changes);
    assert!(!matches.is_empty(), "should detect SendGrid key");
    assert_eq!(matches[0].pattern_name, "SendGrid Key");
}

// ─── build_patterns tests ────────────────────────────────────────────────────

#[test]
fn build_patterns_default_has_all_builtins() {
    let patterns = build_patterns(&[], &[]);
    assert_eq!(
        patterns.len(),
        24,
        "expected exactly 24 built-in patterns, got {}",
        patterns.len()
    );
}

#[test]
fn build_patterns_disable_removes_pattern() {
    let patterns = build_patterns(&[], &["AWS Access Key".to_string()]);
    assert!(
        !patterns.iter().any(|p| p.name == "AWS Access Key"),
        "disabled pattern should be removed"
    );
}

#[test]
fn build_patterns_disable_case_insensitive() {
    let patterns = build_patterns(&[], &["aws access key".to_string()]);
    assert!(
        !patterns.iter().any(|p| p.name == "AWS Access Key"),
        "case-insensitive disable should work"
    );
}

#[test]
fn build_patterns_custom_adds_pattern() {
    let patterns = build_patterns(&["CUSTOM_[a-z]{10}".to_string()], &[]);
    let has_custom = patterns
        .iter()
        .any(|p| p.name.starts_with("Custom Pattern"));
    assert!(has_custom, "custom pattern should be added");
}

#[test]
fn build_patterns_custom_invalid_regex_skipped() {
    // Invalid regex should not cause panic, just be skipped
    let patterns = build_patterns(&["[invalid".to_string()], &[]);
    let custom_count = patterns
        .iter()
        .filter(|p| p.name.starts_with("Custom"))
        .count();
    assert_eq!(custom_count, 0, "invalid regex should be silently skipped");
}

#[test]
fn custom_pattern_detects_match() {
    let patterns = build_patterns(&["MYTOKEN_[A-Z]{20}".to_string()], &[]);
    let diff = "+MYTOKEN_ABCDEFGHIJKLMNOPQRST\n";
    let changes = make_staged_changes(vec![make_file_change(
        "src/custom.rs",
        ChangeStatus::Modified,
        diff,
        1,
        0,
    )]);
    let matches = scan_for_secrets_with_patterns(&changes, &patterns);
    assert!(!matches.is_empty(), "custom pattern should detect match");
    assert!(matches[0].pattern_name.starts_with("Custom Pattern"));
}

#[test]
fn disabled_pattern_not_detected() {
    let patterns = build_patterns(&[], &["Generic API Key".to_string()]);
    let diff = "+API_KEY=abcdefghijklmnopqrstuvwxyz1234567890abcdef\n";
    let full_diff = format!(
        "diff --git a/src/x.rs b/src/x.rs\n--- a/src/x.rs\n+++ b/src/x.rs\n@@ -1,1 +1,2 @@\n fn x() {{}}\n{diff}"
    );
    let matches = scan_full_diff_with_patterns(&full_diff, &patterns);
    assert!(
        !matches.iter().any(|m| m.pattern_name == "Generic API Key"),
        "disabled pattern should not trigger"
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
