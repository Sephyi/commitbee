// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::CommitFormat;
use crate::error::{Error, Result};

/// Structured commit message from LLM (preferred format)
#[derive(Debug, Deserialize, Serialize)]
pub struct StructuredCommit {
    #[serde(rename = "type")]
    pub commit_type: String,
    pub scope: Option<String>,
    pub subject: String,
    pub body: Option<String>,
}

/// Allowed commit types
const VALID_TYPES: &[&str] = &[
    "feat", "fix", "refactor", "chore", "docs", "test", "style", "perf", "build", "ci", "revert",
];

static SCOPE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9\-_/.]*$").unwrap());

static CODE_FENCE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"```[\s\S]*?```").unwrap());

static PREAMBLE_PATTERNS: &[&str] = &[
    "here's the commit message",
    "here is the commit message",
    "commit message:",
    "suggested commit:",
];

pub struct CommitSanitizer;

impl CommitSanitizer {
    /// Parse and validate commit message from LLM output
    pub fn sanitize(raw: &str, format: &CommitFormat) -> Result<String> {
        // Step 1: Try to parse as JSON (structured output)
        if let Ok(structured) = Self::try_parse_json(raw) {
            return Self::format_structured(&structured, format);
        }

        // Step 2: Clean up plain text output
        let cleaned = Self::clean_text(raw, format);

        // Step 3: Validate conventional commit format
        Self::validate_conventional(&cleaned)?;

        Ok(cleaned)
    }

    fn try_parse_json(raw: &str) -> std::result::Result<StructuredCommit, ()> {
        let trimmed = raw.trim();

        // Direct JSON
        if trimmed.starts_with('{') {
            return serde_json::from_str(trimmed).map_err(|_| ());
        }

        // JSON in code fence
        if let Some(start) = trimmed.find("```json") {
            let after_fence = &trimmed[start + 7..];
            if let Some(end) = after_fence.find("```") {
                let json = after_fence[..end].trim();
                return serde_json::from_str(json).map_err(|_| ());
            }
        }

        // Plain code fence
        if let Some(start) = trimmed.find("```") {
            let after_fence = &trimmed[start + 3..];
            if let Some(end) = after_fence.find("```") {
                let content = after_fence[..end].trim();
                if content.starts_with('{') {
                    return serde_json::from_str(content).map_err(|_| ());
                }
            }
        }

        Err(())
    }

    fn format_structured(s: &StructuredCommit, format: &CommitFormat) -> Result<String> {
        // Validate type
        let commit_type = s.commit_type.to_lowercase();
        if !VALID_TYPES.contains(&commit_type.as_str()) {
            return Err(Error::InvalidCommitMessage(format!(
                "Invalid commit type: '{}'. Must be one of: {}",
                commit_type,
                VALID_TYPES.join(", ")
            )));
        }

        // Validate and sanitize scope (only if we're using scopes)
        let scope = if format.include_scope {
            if let Some(ref raw_scope) = s.scope {
                // Sanitize scope: lowercase, replace spaces with hyphens
                let sanitized = raw_scope
                    .to_lowercase()
                    .replace(' ', "-")
                    .replace("--", "-");

                if sanitized.is_empty() {
                    None
                } else if !SCOPE_REGEX.is_match(&sanitized) {
                    // If still invalid after sanitization, skip scope rather than error
                    None
                } else {
                    Some(sanitized)
                }
            } else {
                None
            }
        } else {
            None
        };

        // Format subject: optionally lowercase first char, no period
        let subject = {
            let trimmed = s.subject.trim().trim_end_matches('.');
            if format.lowercase_subject {
                let mut chars = trimmed.chars();
                match chars.next() {
                    Some(first) => first.to_lowercase().chain(chars).collect(),
                    None => String::new(),
                }
            } else {
                trimmed.to_string()
            }
        };

        // Build first line
        let first_line = match scope {
            Some(ref sc) => format!("{}({}): {}", commit_type, sc, subject),
            None => format!("{}: {}", commit_type, subject),
        };

        // Truncate if too long
        let first_line = if first_line.len() > 72 {
            format!("{}...", &first_line[..69])
        } else {
            first_line
        };

        // Add body if present and enabled
        let message = if format.include_body {
            match &s.body {
                Some(body) if !body.trim().is_empty() => {
                    format!("{}\n\n{}", first_line, body.trim())
                }
                _ => first_line,
            }
        } else {
            first_line
        };

        Ok(message)
    }

    fn clean_text(raw: &str, format: &CommitFormat) -> String {
        let mut cleaned = raw.to_string();

        // Remove code fences
        cleaned = CODE_FENCE_REGEX.replace_all(&cleaned, "").to_string();

        // Remove quotes at start/end
        cleaned = cleaned.trim().to_string();
        if cleaned.starts_with('"') && cleaned.ends_with('"') {
            cleaned = cleaned[1..cleaned.len() - 1].to_string();
        }
        if cleaned.starts_with('\'') && cleaned.ends_with('\'') {
            cleaned = cleaned[1..cleaned.len() - 1].to_string();
        }

        // Remove common preambles (case insensitive)
        let lower = cleaned.to_lowercase();
        for pattern in PREAMBLE_PATTERNS {
            if let Some(pos) = lower.find(pattern) {
                let after = &cleaned[pos + pattern.len()..];
                cleaned = after.trim_start_matches(':').trim().to_string();
            }
        }

        // Apply lowercase to subject if enabled (for plain text, lowercase after the colon)
        if format.lowercase_subject {
            if let Some(colon_pos) = cleaned.find(": ") {
                let (prefix, rest) = cleaned.split_at(colon_pos + 2);
                let mut chars = rest.chars();
                if let Some(first) = chars.next() {
                    let lowered: String = first.to_lowercase().chain(chars).collect();
                    cleaned = format!("{}{}", prefix, lowered);
                }
            }
        }

        // Ensure first line <= 72 chars
        if let Some(first_newline) = cleaned.find('\n') {
            let first_line = &cleaned[..first_newline];
            if first_line.len() > 72 {
                let truncated = format!("{}...", &first_line[..69]);
                cleaned = format!("{}{}", truncated, &cleaned[first_newline..]);
            }
        } else if cleaned.len() > 72 {
            cleaned = format!("{}...", &cleaned[..69]);
        }

        cleaned
    }

    fn validate_conventional(message: &str) -> Result<()> {
        let first_line = message.lines().next().unwrap_or("");

        // Check for type prefix
        let has_valid_type = VALID_TYPES.iter().any(|t| {
            first_line.starts_with(&format!("{}:", t))
                || first_line.starts_with(&format!("{}(", t))
        });

        if !has_valid_type {
            return Err(Error::InvalidCommitMessage(format!(
                "Message doesn't start with a valid type. Got: '{}'",
                first_line.chars().take(20).collect::<String>()
            )));
        }

        Ok(())
    }
}
