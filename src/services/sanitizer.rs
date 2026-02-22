// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::CommitFormat;
use crate::domain::CommitType;
use crate::error::{Error, Result};

/// Structured commit message from LLM (preferred format)
#[derive(Debug, Deserialize, Serialize)]
pub struct StructuredCommit {
    #[serde(rename = "type")]
    pub commit_type: String,
    pub scope: Option<String>,
    pub subject: String,
    pub body: Option<String>,
    pub breaking_change: Option<String>, // null or omitted = non-breaking
}

static SCOPE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9\-_/.]*$").unwrap());

static CODE_FENCE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"```[\s\S]*?```").unwrap());

static PREAMBLE_PATTERNS: &[&str] = &[
    "here's the commit message",
    "here is the commit message",
    "commit message:",
    "suggested commit:",
];

pub struct CommitSanitizer;

impl CommitSanitizer {
    /// Word-wrap text to at most `max_width` characters per line.
    /// Preserves existing newlines (paragraph breaks). Handles words longer
    /// than `max_width` by placing them on their own line (no mid-word break).
    fn wrap_body(text: &str, max_width: usize) -> String {
        let mut result = String::new();

        for (i, paragraph) in text.split('\n').enumerate() {
            if i > 0 {
                result.push('\n');
            }

            let trimmed = paragraph.trim();
            if trimmed.is_empty() {
                continue;
            }

            let mut line_len = 0;
            for (j, word) in trimmed.split_whitespace().enumerate() {
                let word_len = word.chars().count();

                if j == 0 {
                    // First word on the line
                    result.push_str(word);
                    line_len = word_len;
                } else if line_len + 1 + word_len > max_width {
                    // Word would exceed line limit — wrap
                    result.push('\n');
                    result.push_str(word);
                    line_len = word_len;
                } else {
                    // Word fits on current line
                    result.push(' ');
                    result.push_str(word);
                    line_len += 1 + word_len;
                }
            }
        }

        result
    }

    /// Truncate a string to at most `max_chars` characters, appending "..." if truncated.
    /// Safe for multi-byte UTF-8 (never slices mid-character).
    fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
        let suffix = "...";
        let target = max_chars.saturating_sub(suffix.len());
        let boundary = s
            .char_indices()
            .nth(target)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        if boundary < s.len() {
            format!("{}{}", &s[..boundary], suffix)
        } else {
            s.to_string()
        }
    }

    /// Format a breaking change description as a git-trailer-safe footer.
    ///
    /// Output:
    ///   `BREAKING CHANGE: <first segment of description>`
    ///   `  <continuation lines, indented two spaces>`
    ///
    /// `str::len()` is `const fn` since Rust 1.39 — `FIRST_LINE_BUDGET` is a
    /// valid compile-time constant on MSRV 1.85.
    fn format_breaking_footer(desc: &str) -> String {
        const PREFIX: &str = "BREAKING CHANGE: ";
        const FIRST_LINE_BUDGET: usize = 72 - PREFIX.len(); // 55

        let wrapped = Self::wrap_body(desc.trim(), FIRST_LINE_BUDGET);
        let mut lines = wrapped.lines();
        let first = lines.next().unwrap_or_default();
        let mut footer = format!("{}{}", PREFIX, first);
        for line in lines {
            footer.push('\n');
            footer.push_str("  ");
            footer.push_str(line);
        }
        footer
    }

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
        if !CommitType::ALL.contains(&commit_type.as_str()) {
            return Err(Error::InvalidCommitMessage(format!(
                "Invalid commit type: '{}'. Must be one of: {}",
                commit_type,
                CommitType::ALL.join(", ")
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

        // Normalize breaking_change: empty/whitespace-only/literal-"null" → None (5a)
        let breaking_change: Option<String> = s
            .breaking_change
            .as_deref()
            .filter(|bc| {
                let t = bc.trim();
                !t.is_empty() && !t.eq_ignore_ascii_case("null")
            })
            .map(|bc| bc.trim().to_string());
        let is_breaking = breaking_change.is_some();

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

        // Build first line with optional ! for breaking changes (5b)
        let bang = if is_breaking { "!" } else { "" };
        let first_line = match scope {
            Some(ref sc) => format!("{}({}){}: {}", commit_type, sc, bang, subject),
            None => format!("{}{}: {}", commit_type, bang, subject),
        };

        // Truncate if too long
        let first_line = if first_line.chars().count() > 72 {
            Self::truncate_with_ellipsis(&first_line, 72)
        } else {
            first_line
        };

        // Body gated by include_body; footer always emitted when breaking (D4, 5d)
        let body_section: Option<String> = if format.include_body {
            match &s.body {
                Some(body) if !body.trim().is_empty() => Some(Self::wrap_body(body.trim(), 72)),
                _ => None,
            }
        } else {
            None
        };

        let footer_section: Option<String> =
            breaking_change.as_deref().map(Self::format_breaking_footer);

        let message = match (body_section, footer_section) {
            (Some(body), Some(footer)) => format!("{}\n\n{}\n\n{}", first_line, body, footer),
            (Some(body), None) => format!("{}\n\n{}", first_line, body),
            (None, Some(footer)) => format!("{}\n\n{}", first_line, footer),
            (None, None) => first_line,
        };

        Ok(message)
    }

    fn clean_text(raw: &str, format: &CommitFormat) -> String {
        let mut cleaned = raw.to_string();

        // Remove code fences
        cleaned = CODE_FENCE_REGEX.replace_all(&cleaned, "").to_string();

        // Remove quotes at start/end
        cleaned = cleaned.trim().to_string();
        if cleaned.starts_with('"') && cleaned.ends_with('"') && cleaned.len() >= 2 {
            cleaned = cleaned[1..cleaned.len() - 1].to_string();
        }
        if cleaned.starts_with('\'') && cleaned.ends_with('\'') && cleaned.len() >= 2 {
            cleaned = cleaned[1..cleaned.len() - 1].to_string();
        }

        // Remove common preambles (case insensitive)
        for pattern in PREAMBLE_PATTERNS {
            let lower = cleaned.to_lowercase();
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
            if first_line.chars().count() > 72 {
                let truncated = Self::truncate_with_ellipsis(first_line, 72);
                cleaned = format!("{}{}", truncated, &cleaned[first_newline..]);
            }
        } else if cleaned.chars().count() > 72 {
            cleaned = Self::truncate_with_ellipsis(&cleaned, 72);
        }

        cleaned
    }

    fn validate_conventional(message: &str) -> Result<()> {
        let first_line = message.lines().next().unwrap_or("");

        // Check for type prefix
        let has_valid_type = CommitType::ALL.iter().any(|t| {
            first_line.starts_with(&format!("{}:", t))        // feat: subject
                || first_line.starts_with(&format!("{}(", t)) // feat(scope): or feat(scope)!:
                || first_line.starts_with(&format!("{}!", t)) // feat!: subject
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
