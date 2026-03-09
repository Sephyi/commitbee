// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::sync::LazyLock;

use regex::Regex;

use crate::domain::StagedChanges;

pub struct SecretMatch {
    pub pattern_name: String,
    pub file: String,
    pub line: Option<usize>,
}

static SECRET_PATTERNS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    vec![
        (
            "API Key",
            Regex::new(r#"(?i)(api[_-]?key|apikey)\s*[:=]\s*["']?[a-zA-Z0-9_-]{20,}"#).unwrap(),
        ),
        ("AWS Key", Regex::new(r"AKIA[0-9A-Z]{16}").unwrap()),
        (
            "Private Key",
            Regex::new(r"-----BEGIN .* PRIVATE KEY-----").unwrap(),
        ),
        (
            "OpenAI Key",
            Regex::new(r"sk-[a-zA-Z0-9]{48}|sk-proj-[a-zA-Z0-9\-_]{40,}").unwrap(),
        ),
        (
            "Anthropic Key",
            Regex::new(r"sk-ant-[a-zA-Z0-9-]{80,}").unwrap(),
        ),
        (
            "GitHub Token",
            Regex::new(r"gh[ps]_[a-zA-Z0-9]{36,}").unwrap(),
        ),
        (
            "Generic Secret",
            Regex::new(r#"(?i)(password|secret|token)\s*[:=]\s*["'][^"']{8,}["']"#).unwrap(),
        ),
        (
            "Generic Secret (unquoted)",
            Regex::new(r#"(?i)(password|secret|token)\s*[:=]\s*[^\s'"]{16,}"#).unwrap(),
        ),
        (
            "Connection String",
            Regex::new(r"(?i)(mongodb|postgres|mysql|redis)://[^\s]+").unwrap(),
        ),
    ]
});

/// Scan per-file truncated diffs for secrets.
///
/// Prefer `scan_full_diff_for_secrets` in the main binary — it catches
/// secrets beyond `max_file_lines` truncation. This function is retained
/// for library consumers who only have `StagedChanges`.
#[allow(dead_code)]
pub fn scan_for_secrets(changes: &StagedChanges) -> Vec<SecretMatch> {
    let mut found = Vec::new();

    for file in &changes.files {
        if file.is_binary {
            continue;
        }

        let mut line_num = 0;
        for line in file.diff.lines() {
            line_num += 1;

            // Only check added lines
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }

            for (name, pattern) in SECRET_PATTERNS.iter() {
                if pattern.is_match(line) {
                    found.push(SecretMatch {
                        pattern_name: name.to_string(),
                        file: file.path.display().to_string(),
                        line: Some(line_num),
                    });
                    break; // One match per line is enough
                }
            }
        }
    }

    found
}

/// Scan the full unified diff for secrets (before per-file truncation).
///
/// This catches secrets that would be missed by `scan_for_secrets` when
/// file diffs are truncated to `max_file_lines`. Parses the raw `git diff`
/// output directly, tracking file paths from diff headers.
pub fn scan_full_diff_for_secrets(full_diff: &str) -> Vec<SecretMatch> {
    let mut found = Vec::new();
    let mut current_file = String::new();
    let mut line_num: usize = 0;

    for line in full_diff.lines() {
        // Track current file from diff headers
        if line.starts_with("diff --git ") {
            line_num = 0;
            continue;
        }

        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file = path.to_string();
            continue;
        }

        if line == "+++ /dev/null" {
            // Deleted file — keep current_file from --- header
            continue;
        }

        if let Some(path) = line.strip_prefix("--- a/") {
            // For deleted files, this is the only file path we get
            if current_file.is_empty() {
                current_file = path.to_string();
            }
            continue;
        }

        line_num += 1;

        // Only check added lines
        if !line.starts_with('+') || line.starts_with("+++") {
            continue;
        }

        for (name, pattern) in SECRET_PATTERNS.iter() {
            if pattern.is_match(line) {
                found.push(SecretMatch {
                    pattern_name: name.to_string(),
                    file: current_file.clone(),
                    line: Some(line_num),
                });
                break;
            }
        }
    }

    found
}

/// Check for merge conflict markers
/// Note: This can false-positive in docs/test fixtures, so treat as warning
pub fn check_for_conflicts(changes: &StagedChanges) -> bool {
    for file in &changes.files {
        // Skip docs/test files where conflict markers might be intentional examples
        // Use path components to avoid matching "testing_utils" or "documentation" substrings
        if file.path.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            s == "tests" || s == "docs" || s == "examples" || s.contains("test")
        }) {
            continue;
        }

        // Only check added lines for conflict markers
        for line in file.diff.lines() {
            if line.starts_with('+') && !line.starts_with("+++") {
                // Split strings to prevent self-detection in this file's own diff
                const CONFLICT_START: &str = concat!("<", "<<<<<<");
                const CONFLICT_END: &str = concat!(">", ">>>>>>");

                if line.contains(CONFLICT_START) || line.contains(CONFLICT_END) {
                    return true;
                }
            }
        }
    }
    false
}
