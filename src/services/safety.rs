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
        ("OpenAI Key", Regex::new(r"sk-[a-zA-Z0-9]{48}").unwrap()),
        (
            "Anthropic Key",
            Regex::new(r"sk-ant-[a-zA-Z0-9-]{80,}").unwrap(),
        ),
        (
            "Generic Secret",
            Regex::new(r#"(?i)(password|secret|token)\s*[:=]\s*["'][^"']{8,}["']"#).unwrap(),
        ),
        (
            "Connection String",
            Regex::new(r"(?i)(mongodb|postgres|mysql|redis)://[^\s]+").unwrap(),
        ),
    ]
});

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

/// Check for merge conflict markers
/// Note: This can false-positive in docs/test fixtures, so treat as warning
pub fn check_for_conflicts(changes: &StagedChanges) -> bool {
    for file in &changes.files {
        // Skip docs/test files where conflict markers might be intentional examples
        if file.path.to_string_lossy().contains("test")
            || file.path.to_string_lossy().contains("doc")
            || file.path.to_string_lossy().contains("example")
        {
            continue;
        }

        if file.diff.contains("<<<<<<<") || file.diff.contains(">>>>>>>") {
            return true;
        }
    }
    false
}
