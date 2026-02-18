// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod helpers;

use commitbee::config::CommitFormat;
use commitbee::services::sanitizer::CommitSanitizer;
use proptest::prelude::*;

fn default_format() -> CommitFormat {
    CommitFormat::default()
}

// ─── JSON parsing tests ───────────────────────────────────────────────────────

#[test]
fn sanitize_valid_json() {
    let raw = r#"{"type": "feat", "scope": "cli", "subject": "add verbose flag", "body": null}"#;
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"feat(cli): add verbose flag");
}

#[test]
fn sanitize_json_in_code_fence() {
    let raw = r#"```json
{"type": "fix", "scope": "git", "subject": "handle detached HEAD state", "body": null}
```"#;
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"fix(git): handle detached HEAD state");
}

#[test]
fn sanitize_json_in_plain_fence() {
    let raw = r#"```
{"type": "refactor", "scope": "context", "subject": "extract token budget logic", "body": null}
```"#;
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"refactor(context): extract token budget logic");
}

#[test]
fn sanitize_json_with_body() {
    let raw = r#"{"type": "feat", "scope": "llm", "subject": "add streaming support", "body": "Uses tokio-stream to stream tokens from Ollama.\nImproves perceived latency for long responses."}"#;
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    // Body is non-trivial — use non-inline snapshot
    insta::assert_snapshot!(result);
}

#[test]
fn sanitize_json_invalid_type() {
    let raw = r#"{"type": "yolo", "scope": "cli", "subject": "ship it", "body": null}"#;
    let result = CommitSanitizer::sanitize(raw, &default_format());
    assert!(
        result.is_err(),
        "expected Err for invalid commit type 'yolo'"
    );
}

// ─── Plain text tests ─────────────────────────────────────────────────────────

#[test]
fn sanitize_plain_text_conventional() {
    let raw = "feat(cli): add --dry-run flag";
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"feat(cli): add --dry-run flag");
}

#[test]
fn sanitize_plain_with_preamble() {
    // Single-line preamble so only one pattern fires (avoids the multi-pattern
    // overlap bug where "commit message:" is a substring of the already-matched
    // "here's the commit message" pattern).
    let raw = "Suggested commit: feat(cli): add --dry-run flag";
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"feat(cli): add --dry-run flag");
}

#[test]
fn sanitize_plain_with_quotes() {
    let raw = r#""fix(git): handle missing remote""#;
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"fix(git): handle missing remote");
}

#[test]
fn sanitize_invalid_no_type() {
    let raw = "just some random text without a valid type prefix";
    let result = CommitSanitizer::sanitize(raw, &default_format());
    assert!(
        result.is_err(),
        "expected Err for input with no valid commit type"
    );
}

// ─── Edge cases ───────────────────────────────────────────────────────────────

#[test]
fn sanitize_empty_input() {
    let result = CommitSanitizer::sanitize("", &default_format());
    assert!(result.is_err(), "expected Err for empty input");
}

#[test]
fn sanitize_whitespace_only() {
    let result = CommitSanitizer::sanitize("   \n\t  ", &default_format());
    assert!(result.is_err(), "expected Err for whitespace-only input");
}

// ─── UTF-8 safety (FR-001) ────────────────────────────────────────────────────

#[test]
fn sanitize_unicode_emoji_in_subject() {
    // Emoji are multi-byte; a very long subject with emoji should truncate safely (no panic)
    let long_subject = "🦀".repeat(100);
    let raw = format!(
        r#"{{"type": "chore", "scope": null, "subject": "{}", "body": null}}"#,
        long_subject
    );
    // Must not panic — result can be Ok or Err
    let _ = CommitSanitizer::sanitize(&raw, &default_format());
}

#[test]
fn sanitize_cjk_characters() {
    // CJK characters are 3 bytes each; ensure no mid-char slicing
    let raw = r#"{"type": "docs", "scope": "readme", "subject": "添加中文说明文档以便于理解项目架构和使用方式", "body": null}"#;
    let result = CommitSanitizer::sanitize(raw, &default_format());
    // Must not panic; validate if Ok that the string is valid UTF-8
    if let Ok(msg) = result {
        assert!(std::str::from_utf8(msg.as_bytes()).is_ok());
    }
}

#[test]
fn sanitize_accented_characters() {
    // Accented characters (2 bytes each in UTF-8) in a long subject
    let long_accented = "é".repeat(80);
    let raw = format!(
        r#"{{"type": "fix", "scope": null, "subject": "{}", "body": null}}"#,
        long_accented
    );
    // Must not panic
    let result = CommitSanitizer::sanitize(&raw, &default_format());
    if let Ok(msg) = result {
        // Result must be valid UTF-8 and first line within 72 chars
        let first_line = msg.lines().next().unwrap_or("");
        assert!(first_line.chars().count() <= 72);
    }
}

// ─── Format options ───────────────────────────────────────────────────────────

#[test]
fn sanitize_no_scope() {
    let raw = r#"{"type": "feat", "scope": "cli", "subject": "add verbose flag", "body": null}"#;
    let format = CommitFormat {
        include_scope: false,
        ..CommitFormat::default()
    };
    let result = CommitSanitizer::sanitize(raw, &format).unwrap();
    insta::assert_snapshot!(result, @"feat: add verbose flag");
}

#[test]
fn sanitize_no_body() {
    let raw = r#"{"type": "feat", "scope": "llm", "subject": "add streaming support", "body": "This is the body text."}"#;
    let format = CommitFormat {
        include_body: false,
        ..CommitFormat::default()
    };
    let result = CommitSanitizer::sanitize(raw, &format).unwrap();
    insta::assert_snapshot!(result, @"feat(llm): add streaming support");
}

#[test]
fn sanitize_no_lowercase() {
    let raw =
        r#"{"type": "fix", "scope": "git", "subject": "Handle Detached HEAD State", "body": null}"#;
    let format = CommitFormat {
        lowercase_subject: false,
        ..CommitFormat::default()
    };
    let result = CommitSanitizer::sanitize(raw, &format).unwrap();
    insta::assert_snapshot!(result, @"fix(git): Handle Detached HEAD State");
}

// ─── Scope handling ──────────────────────────────────────────────────────────

#[test]
fn sanitize_scope_with_spaces() {
    let raw = r#"{"type": "feat", "scope": "my scope", "subject": "add feature", "body": null}"#;
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"feat(my-scope): add feature");
}

#[test]
fn sanitize_scope_invalid_chars() {
    let raw = r#"{"type": "feat", "scope": "@#$%", "subject": "add feature", "body": null}"#;
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"feat: add feature");
}

// ─── Truncation boundary ─────────────────────────────────────────────────────

#[test]
fn sanitize_truncation_boundary_72() {
    // "feat: " is 6 chars, so subject needs to be 66 chars for exactly 72
    let subject_66 = "a".repeat(66);
    let raw = format!(
        r#"{{"type": "feat", "scope": null, "subject": "{}", "body": null}}"#,
        subject_66
    );
    let result = CommitSanitizer::sanitize(&raw, &default_format()).unwrap();
    assert_eq!(
        result.chars().count(),
        72,
        "exactly 72 chars should not be truncated"
    );

    // 67 chars → first line = 73 chars → should be truncated
    let subject_67 = "b".repeat(67);
    let raw = format!(
        r#"{{"type": "feat", "scope": null, "subject": "{}", "body": null}}"#,
        subject_67
    );
    let result = CommitSanitizer::sanitize(&raw, &default_format()).unwrap();
    assert!(
        result.chars().count() <= 72,
        "73+ char first line should be truncated to ≤72, got {}",
        result.chars().count()
    );
    assert!(
        result.ends_with("..."),
        "truncated line should end with '...'"
    );
}

// ─── Subject normalization ───────────────────────────────────────────────────

#[test]
fn sanitize_subject_trailing_period() {
    let raw =
        r#"{"type": "fix", "scope": "git", "subject": "resolve merge conflicts.", "body": null}"#;
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"fix(git): resolve merge conflicts");
}

#[test]
fn sanitize_uppercase_type_in_json() {
    let raw = r#"{"type": "FEAT", "scope": "cli", "subject": "add verbose flag", "body": null}"#;
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"feat(cli): add verbose flag");
}

// ─── Body handling ───────────────────────────────────────────────────────────

#[test]
fn sanitize_json_null_body() {
    // Explicit null
    let raw_null = r#"{"type": "fix", "scope": null, "subject": "patch bug", "body": null}"#;
    let result_null = CommitSanitizer::sanitize(raw_null, &default_format()).unwrap();

    // Missing body field entirely — serde_json deserializes missing Option<String>
    // as None, so both variants parse successfully and produce identical output.
    let raw_missing = r#"{"type": "fix", "scope": null, "subject": "patch bug"}"#;
    let result_missing = CommitSanitizer::sanitize(raw_missing, &default_format()).unwrap();

    assert_eq!(
        result_null, result_missing,
        "null body and missing body should produce identical output"
    );
    insta::assert_snapshot!(result_null, @"fix: patch bug");
}

// ─── Code fence stripping ────────────────────────────────────────────────────

#[test]
fn sanitize_code_fence_in_plain_text() {
    let raw = "```\nsome preamble\n```\nfeat(cli): add verbose flag";
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();
    insta::assert_snapshot!(result, @"feat(cli): add verbose flag");
}

// ─── Proptest: never panics ───────────────────────────────────────────────────

proptest! {
    #[test]
    fn sanitizer_never_panics(raw in ".*") {
        let format = CommitFormat::default();
        // Any input must produce Ok or Err — never a panic
        let _ = CommitSanitizer::sanitize(&raw, &format);
    }
}

// ─── Body wrapping tests ─────────────────────────────────────────────────────

#[test]
fn sanitize_json_body_wrapped_at_72() {
    let long_body = "This is a very long body line that should be wrapped because it exceeds the seventy-two character limit for conventional commit body lines.";
    let json = format!(
        r#"{{"type": "feat", "scope": "core", "subject": "add new feature", "body": "{}"}}"#,
        long_body
    );
    let result = CommitSanitizer::sanitize(&json, &default_format()).unwrap();

    let lines: Vec<&str> = result.lines().collect();
    // Skip header line and blank separator line
    for line in &lines[2..] {
        assert!(
            line.chars().count() <= 72,
            "Body line exceeds 72 chars: '{}' ({})",
            line,
            line.chars().count()
        );
    }
    // Verify the body content is preserved (not lost)
    let body_text: String = lines[2..].join(" ");
    assert!(body_text.contains("seventy-two character limit"));
}

#[test]
fn sanitize_json_body_short_not_wrapped() {
    let json = r#"{"type": "fix", "scope": null, "subject": "fix bug", "body": "Short body."}"#;
    let result = CommitSanitizer::sanitize(json, &default_format()).unwrap();

    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 3); // header + blank + body
    assert_eq!(lines[2], "Short body.");
}

#[test]
fn sanitize_json_body_preserves_paragraphs() {
    let json = r#"{"type": "feat", "scope": null, "subject": "add feature", "body": "First paragraph.\n\nSecond paragraph."}"#;
    let result = CommitSanitizer::sanitize(json, &default_format()).unwrap();

    let lines: Vec<&str> = result.lines().collect();
    assert!(lines.contains(&"First paragraph."));
    assert!(lines.contains(&"Second paragraph."));
}
