// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use crate::domain::StagedChanges;

pub struct SecretMatch {
    pub pattern_name: String,
    pub file: String,
    pub line: Option<usize>,
}

/// A named secret detection pattern with description.
///
/// `Regex` clones are cheap (internally reference-counted), and the
/// string fields are `Cow<'static, str>`, so cloning a `SecretPattern`
/// from the cached built-in set avoids recompiling the underlying regex.
#[allow(dead_code)]
#[derive(Clone)]
pub struct SecretPattern {
    pub name: Cow<'static, str>,
    pub regex: Regex,
    pub description: Cow<'static, str>,
}

/// Build the full set of secret patterns, applying custom additions and disabled names.
///
/// Custom patterns are compiled from user-provided regex strings. Invalid regexes
/// are silently skipped (logged at warn level in the caller).
///
/// The 24 built-in patterns are compiled exactly once per process (cached in
/// `BUILTIN_PATTERNS`); each call clones the cached entries rather than
/// recompiling regexes.
pub fn build_patterns(custom: &[String], disabled: &[String]) -> Vec<SecretPattern> {
    let builtin = builtin_patterns();

    // Remove disabled patterns by name (case-insensitive match)
    let mut patterns: Vec<SecretPattern> = if disabled.is_empty() {
        builtin.to_vec()
    } else {
        let disabled_lower: Vec<String> = disabled.iter().map(|s| s.to_lowercase()).collect();
        builtin
            .iter()
            .filter(|p| !disabled_lower.contains(&p.name.to_lowercase()))
            .cloned()
            .collect()
    };

    // Add custom patterns
    for (i, raw) in custom.iter().enumerate() {
        if let Ok(regex) = Regex::new(raw) {
            patterns.push(SecretPattern {
                name: Cow::Owned(format!("Custom Pattern {}", i + 1)),
                regex,
                description: Cow::Owned(format!("User-defined: {}", raw)),
            });
        }
    }

    patterns
}

/// Return the cached built-in pattern set.
///
/// The underlying `Vec<SecretPattern>` is built once on first access via
/// `LazyLock` and then shared for the lifetime of the process.
fn builtin_patterns() -> &'static [SecretPattern] {
    &BUILTIN_PATTERNS
}

/// Cached built-in secret patterns. Compiled exactly once per process.
static BUILTIN_PATTERNS: LazyLock<Vec<SecretPattern>> = LazyLock::new(|| {
    vec![
        // ── Cloud Provider API Keys ──
        SecretPattern {
            name: Cow::Borrowed("AWS Access Key"),
            regex: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
            description: Cow::Borrowed("AWS IAM access key ID"),
        },
        SecretPattern {
            name: Cow::Borrowed("AWS Secret Key"),
            regex: Regex::new(
                r#"(?i)aws[_-]?secret[_-]?access[_-]?key\s*[:=]\s*["']?[A-Za-z0-9/+=]{40}"#,
            )
            .unwrap(),
            description: Cow::Borrowed("AWS secret access key"),
        },
        SecretPattern {
            name: Cow::Borrowed("GCP Service Account"),
            regex: Regex::new(r#""type"\s*:\s*"service_account""#).unwrap(),
            description: Cow::Borrowed("Google Cloud service account JSON key"),
        },
        SecretPattern {
            name: Cow::Borrowed("GCP API Key"),
            regex: Regex::new(r"AIza[0-9A-Za-z_-]{35}").unwrap(),
            description: Cow::Borrowed("Google API key"),
        },
        SecretPattern {
            name: Cow::Borrowed("Azure Storage Key"),
            regex: Regex::new(r#"(?i)AccountKey\s*=\s*[A-Za-z0-9+/=]{44,}"#).unwrap(),
            description: Cow::Borrowed("Azure storage account key"),
        },
        // ── AI/ML Provider Keys ──
        SecretPattern {
            name: Cow::Borrowed("OpenAI Key"),
            regex: Regex::new(r"sk-(?:proj-|svcacct-)[a-zA-Z0-9\-_]{20,}|sk-[a-zA-Z0-9]{48}")
                .unwrap(),
            description: Cow::Borrowed(
                "OpenAI API key (legacy, project-scoped, or service account)",
            ),
        },
        SecretPattern {
            name: Cow::Borrowed("Anthropic Key"),
            regex: Regex::new(r"sk-ant-[a-zA-Z0-9-]{80,}").unwrap(),
            description: Cow::Borrowed("Anthropic API key"),
        },
        SecretPattern {
            name: Cow::Borrowed("HuggingFace Token"),
            regex: Regex::new(r"hf_[a-zA-Z0-9]{34,}").unwrap(),
            description: Cow::Borrowed("HuggingFace access token"),
        },
        // ── Source Control & CI ──
        SecretPattern {
            name: Cow::Borrowed("GitHub Token"),
            regex: Regex::new(r"gh[ps]_[a-zA-Z0-9]{36,}").unwrap(),
            description: Cow::Borrowed("GitHub personal access or OAuth token"),
        },
        SecretPattern {
            name: Cow::Borrowed("GitHub Fine-Grained Token"),
            regex: Regex::new(r"github_pat_[a-zA-Z0-9_]{22,}").unwrap(),
            description: Cow::Borrowed("GitHub fine-grained personal access token"),
        },
        SecretPattern {
            name: Cow::Borrowed("GitLab Token"),
            regex: Regex::new(r"glpat-[a-zA-Z0-9_-]{20,}").unwrap(),
            description: Cow::Borrowed("GitLab personal access token"),
        },
        // ── Communication Platforms ──
        SecretPattern {
            name: Cow::Borrowed("Slack Token"),
            regex: Regex::new(r"xox[bpras]-[0-9]{10,}-[a-zA-Z0-9-]+").unwrap(),
            description: Cow::Borrowed("Slack bot, user, or app token"),
        },
        SecretPattern {
            name: Cow::Borrowed("Slack Webhook"),
            regex: Regex::new(
                r"https://hooks\.slack\.com/services/T[0-9A-Z]+/B[0-9A-Z]+/[a-zA-Z0-9]+",
            )
            .unwrap(),
            description: Cow::Borrowed("Slack incoming webhook URL"),
        },
        SecretPattern {
            name: Cow::Borrowed("Discord Webhook"),
            regex: Regex::new(r"https://discord(?:app)?\.com/api/webhooks/\d+/[a-zA-Z0-9_-]+")
                .unwrap(),
            description: Cow::Borrowed("Discord webhook URL"),
        },
        // ── Payment & SaaS ──
        SecretPattern {
            name: Cow::Borrowed("Stripe Key"),
            regex: Regex::new(r"[sr]k_(live|test)_[a-zA-Z0-9]{24,}").unwrap(),
            description: Cow::Borrowed("Stripe secret or restricted API key"),
        },
        SecretPattern {
            name: Cow::Borrowed("Twilio Key"),
            regex: Regex::new(r"SK[a-f0-9]{32}").unwrap(),
            description: Cow::Borrowed("Twilio API key SID"),
        },
        SecretPattern {
            name: Cow::Borrowed("SendGrid Key"),
            regex: Regex::new(r"SG\.[a-zA-Z0-9_-]{22,}\.[a-zA-Z0-9_-]{43,}").unwrap(),
            description: Cow::Borrowed("SendGrid API key"),
        },
        SecretPattern {
            name: Cow::Borrowed("Mailgun Key"),
            regex: Regex::new(r"key-[a-f0-9]{32}").unwrap(),
            description: Cow::Borrowed("Mailgun API key"),
        },
        // ── Database & Infrastructure ──
        SecretPattern {
            name: Cow::Borrowed("Connection String"),
            regex: Regex::new(r"(?i)(mongodb(\+srv)?|postgres(ql)?|mysql|redis|amqp)://[^\s]+")
                .unwrap(),
            description: Cow::Borrowed("Database or message broker connection URI"),
        },
        // ── Cryptographic Material ──
        SecretPattern {
            name: Cow::Borrowed("Private Key"),
            regex: Regex::new(r"-----BEGIN .* PRIVATE KEY-----").unwrap(),
            description: Cow::Borrowed("PEM-encoded private key (RSA, EC, etc.)"),
        },
        SecretPattern {
            name: Cow::Borrowed("JWT Token"),
            regex: Regex::new(r"eyJ[a-zA-Z0-9_-]{10,}\.eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]+")
                .unwrap(),
            description: Cow::Borrowed("JSON Web Token (three-part Base64)"),
        },
        // ── Generic Patterns ──
        SecretPattern {
            name: Cow::Borrowed("Generic API Key"),
            regex: Regex::new(r#"(?i)(api[_-]?key|apikey)\s*[:=]\s*["']?[a-zA-Z0-9_-]{20,}"#)
                .unwrap(),
            description: Cow::Borrowed("Generic API key assignment"),
        },
        SecretPattern {
            name: Cow::Borrowed("Generic Secret"),
            regex: Regex::new(r#"(?i)(password|secret|token)\s*[:=]\s*["'][^"']{8,}["']"#).unwrap(),
            description: Cow::Borrowed("Quoted password, secret, or token assignment"),
        },
        SecretPattern {
            name: Cow::Borrowed("Generic Secret (unquoted)"),
            regex: Regex::new(r#"(?i)(password|secret|token)\s*[:=]\s*[^\s'"]{16,}"#).unwrap(),
            description: Cow::Borrowed("Unquoted password, secret, or token assignment"),
        },
    ]
});

/// Scan per-file truncated diffs for secrets using default patterns.
///
/// Prefer `scan_full_diff_for_secrets` in the main binary — it catches
/// secrets beyond `max_file_lines` truncation. This function is retained
/// for library consumers who only have `StagedChanges`.
#[allow(dead_code)]
pub fn scan_for_secrets(changes: &StagedChanges) -> Vec<SecretMatch> {
    scan_for_secrets_with_patterns(changes, builtin_patterns())
}

/// Scan per-file truncated diffs for secrets using the given pattern set.
pub fn scan_for_secrets_with_patterns(
    changes: &StagedChanges,
    patterns: &[SecretPattern],
) -> Vec<SecretMatch> {
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

            for pat in patterns {
                if pat.regex.is_match(line) {
                    found.push(SecretMatch {
                        pattern_name: pat.name.to_string(),
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

/// Scan the full unified diff for secrets using default patterns.
#[allow(dead_code)]
pub fn scan_full_diff_for_secrets(full_diff: &str) -> Vec<SecretMatch> {
    scan_full_diff_with_patterns(full_diff, builtin_patterns())
}

static HUNK_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@@ -\d+,?\d* \+(\d+),?\d* @@").unwrap());

/// Scan the full unified diff for secrets using the given pattern set.
///
/// This catches secrets that would be missed by `scan_for_secrets` when
/// file diffs are truncated to `max_file_lines`. Parses the raw `git diff`
/// output directly, tracking file paths from diff headers and calculating
/// accurate source line numbers from hunk headers (`@@ -L,l +R,r @@`).
pub fn scan_full_diff_with_patterns(
    full_diff: &str,
    patterns: &[SecretPattern],
) -> Vec<SecretMatch> {
    let mut found = Vec::new();
    let mut current_file = String::new();
    let mut current_line: Option<usize> = None;

    for line in full_diff.lines() {
        // Track current file from diff headers
        if line.starts_with("diff --git ") {
            current_file.clear();
            current_line = None;
            continue;
        }

        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file = path.to_string();
            continue;
        }

        if line == "+++ /dev/null" {
            // Deleted file — keep current_file from --- header if we have one
            continue;
        }

        if let Some(path) = line.strip_prefix("--- a/") {
            // For deleted files, this is the only file path we get
            if current_file.is_empty() {
                current_file = path.to_string();
            }
            continue;
        }

        // Parse hunk header: @@ -1,5 +1,6 @@
        if let Some(caps) = HUNK_HEADER.captures(line)
            && let Ok(start) = caps[1].parse::<usize>()
        {
            current_line = Some(start);
            continue;
        }

        let Some(ref mut line_num) = current_line else {
            continue; // Not inside a hunk
        };

        // Skip diff headers like "index ...", "old mode ...", "--- ...", etc.
        // Also skip "No newline at end of file"
        if line.starts_with('\\') || line.starts_with("index") || line.starts_with("old mode") {
            continue;
        }

        // Only check added lines
        if !line.starts_with("+++")
            && let Some(content) = line.strip_prefix('+')
        {
            for pat in patterns {
                if pat.regex.is_match(content) {
                    found.push(SecretMatch {
                        pattern_name: pat.name.to_string(),
                        file: current_file.clone(),
                        line: Some(*line_num),
                    });
                    break;
                }
            }
            *line_num += 1;
        } else if line.starts_with(' ') {
            // Context line
            *line_num += 1;
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
