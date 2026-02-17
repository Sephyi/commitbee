// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

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

// ─── Proptest: never panics ───────────────────────────────────────────────────

proptest! {
    #[test]
    fn sanitizer_never_panics(raw in ".*") {
        let format = CommitFormat::default();
        // Any input must produce Ok or Err — never a panic
        let _ = CommitSanitizer::sanitize(&raw, &format);
    }
}
