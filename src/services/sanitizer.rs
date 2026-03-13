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

static THOUGHT_BLOCK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<(?:thought|think)>[\s\S]*?</(?:thought|think)>").unwrap());

static UNCLOSED_THOUGHT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*<(?:thought|think)>\s*").unwrap());

static VALID_TYPE_START_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    let types = CommitType::ALL.join("|");
    Regex::new(&format!(r"(?m)(?:^|\s)({})(?:\(|!|:)", types)).unwrap()
});

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

    /// Format a breaking change description as a git-trailer-safe footer.
    ///
    /// Output:
    ///   `BREAKING CHANGE: <first segment of description>`
    ///   `  <continuation lines, indented two spaces>`
    ///
    /// `str::len()` is `const fn` since Rust 1.39 — `FIRST_LINE_BUDGET` is a
    /// valid compile-time constant on MSRV 1.94.
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
            let msg = Self::format_structured(&structured, format)?;
            return Ok(Self::strip_control_chars(&msg));
        }

        // Step 2: Clean up plain text output
        let cleaned = Self::clean_text(raw, format);

        // Step 3: Validate conventional commit format
        Self::validate_conventional(&cleaned)?;

        // Step 4: Validate first line length
        let first_line = cleaned.lines().next().unwrap_or("");
        if first_line.chars().count() > 72 {
            return Err(Error::InvalidCommitMessage(format!(
                "First line is {} chars (max 72): '{}'",
                first_line.chars().count(),
                first_line,
            )));
        }

        Ok(Self::strip_control_chars(&cleaned))
    }

    /// Remove control characters (null bytes, escape sequences, carriage returns)
    /// that could interfere with terminal display or git commit storage.
    /// Preserves newlines and tabs.
    fn strip_control_chars(s: &str) -> String {
        s.chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .collect()
    }

    fn try_parse_json(raw: &str) -> std::result::Result<StructuredCommit, ()> {
        // Strip thought blocks first to avoid picking up braces inside thoughts
        let stripped = THOUGHT_BLOCK_REGEX.replace_all(raw, "");
        let stripped = UNCLOSED_THOUGHT_REGEX.replace(&stripped, "");
        let trimmed = stripped.trim();

        // 1. Look for the start of our specific JSON structure
        // We look for "type" key because that's mandatory and highly specific.
        if let Some(type_key_pos) = trimmed.find("type") {
            // Find the '{' that starts this object (search backwards from "type")
            if let Some(start_brace) = trimmed[..type_key_pos].rfind('{') {
                let json_candidate = &trimmed[start_brace..];
                // Find the last '}' to handle trailing text
                if let Some(end_brace) = json_candidate.rfind('}') {
                    let json = &json_candidate[..=end_brace];
                    if let Ok(structured) = serde_json::from_str::<StructuredCommit>(json) {
                        return Ok(structured);
                    }
                }
            }
        }

        // 2. Fallback to any brace if specific start not found
        if let Some(start_brace) = trimmed.find('{') {
            let json_candidate = &trimmed[start_brace..];
            if let Some(end_brace) = json_candidate.rfind('}') {
                let json = &json_candidate[..=end_brace];
                if let Ok(structured) = serde_json::from_str::<StructuredCommit>(json) {
                    return Ok(structured);
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

        // Reject if too long (validator should have caught this and triggered retry)
        if first_line.chars().count() > 72 {
            return Err(Error::InvalidCommitMessage(format!(
                "First line is {} chars (max 72): '{}'",
                first_line.chars().count(),
                first_line,
            )));
        }

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

        // Remove thought blocks
        cleaned = THOUGHT_BLOCK_REGEX.replace_all(&cleaned, "").to_string();
        cleaned = UNCLOSED_THOUGHT_REGEX.replace(&cleaned, "").to_string();

        // Remove code fences
        cleaned = CODE_FENCE_REGEX.replace_all(&cleaned, "").to_string();

        // Find the actual start of the conventional commit (skip preambles/thoughts)
        if let Some(mat) = VALID_TYPE_START_REGEX.find(&cleaned) {
            cleaned = cleaned[mat.start()..].to_string();
        }

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
        if format.lowercase_subject
            && let Some(colon_pos) = cleaned.find(": ")
        {
            let (prefix, rest) = cleaned.split_at(colon_pos + 2);
            let mut chars = rest.chars();
            if let Some(first) = chars.next() {
                let lowered: String = first.to_lowercase().chain(chars).collect();
                cleaned = format!("{}{}", prefix, lowered);
            }
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

    /// Try to parse raw LLM output as structured JSON without sanitizing.
    /// Used by the post-generation validator to inspect the LLM's raw intent.
    #[must_use]
    pub fn parse_structured(raw: &str) -> Option<StructuredCommit> {
        Self::try_parse_json(raw).ok()
    }
}

/// Post-generation evidence-based validation.
///
/// Checks if a structured commit message is consistent with the evidence flags
/// computed from code analysis. Returns a list of violations that can be used
/// to construct a corrective re-prompt.
pub struct CommitValidator;

impl CommitValidator {
    /// Validate a structured commit against evidence flags.
    /// Returns violations as human-readable correction instructions.
    #[must_use]
    pub fn validate(
        commit: &StructuredCommit,
        has_bug_evidence: bool,
        is_mechanical: bool,
        public_api_removed_count: usize,
        is_dependency_only: bool,
    ) -> Vec<String> {
        let mut violations = Vec::new();
        let commit_type = commit.commit_type.to_lowercase();

        // Rule 1: type=fix requires bug evidence
        if commit_type == "fix" && !has_bug_evidence {
            violations.push(
                "Type is \"fix\" but no bug-fix comments were found in the diff. \
                 Use \"refactor\" instead."
                    .to_string(),
            );
        }

        // Rule 2: breaking_change must be set when public API removed
        if commit.breaking_change.is_none() && public_api_removed_count > 0 {
            violations.push(
                "Public APIs were removed but breaking_change is null. \
                 Describe what was removed in plain English."
                    .to_string(),
            );
        }

        // Rule 3: breaking_change must not copy internal field names
        if let Some(ref bc) = commit.breaking_change {
            let lower = bc.to_lowercase();
            if lower.contains("public_api_removed")
                || lower.contains("bug_evidence")
                || lower.contains("mechanical_transform")
                || lower.contains("dependency_only")
            {
                violations.push(
                    "The breaking_change field contains internal label names. \
                     Describe the actual API change in plain English."
                        .to_string(),
                );
            }
        }

        // Rule 4: mechanical transform cannot be feat or fix
        if is_mechanical && matches!(commit_type.as_str(), "feat" | "fix") {
            violations.push(
                "Change is a mechanical/formatting transform but type is \"feat\"/\"fix\". \
                 Use \"style\" or \"refactor\"."
                    .to_string(),
            );
        }

        // Rule 5: dependency-only should be chore
        if is_dependency_only && commit_type != "chore" {
            violations.push(
                "All changes are in dependency/config files but type is not \"chore\".".to_string(),
            );
        }

        // Rule 6: subject too generic (vague verb + vague noun)
        if Self::is_generic_subject(&commit.subject) {
            violations.push(
                "Subject is too generic — name the specific API, function, or module changed. \
                 Avoid vague verbs like \"update\", \"improve\", \"change\"."
                    .to_string(),
            );
        }

        // Rule 7: subject too long for 72-char first line
        let subject_trimmed = commit.subject.trim().trim_end_matches('.');
        let prefix_len = commit.commit_type.len()
            + commit.scope.as_ref().map(|s| s.len() + 2).unwrap_or(0)
            + if commit.breaking_change.is_some() {
                1
            } else {
                0
            }
            + 2; // ": "
        let first_line_len = prefix_len + subject_trimmed.chars().count();
        if first_line_len > 72 {
            let budget = 72_usize.saturating_sub(prefix_len);
            violations.push(format!(
                "Subject is {} chars but must be under {} chars \
                 (first line would be {} chars, max 72). Shorten it.",
                subject_trimmed.chars().count(),
                budget,
                first_line_len,
            ));
        }

        violations
    }

    /// Check if a subject is too generic (vague verb + vague noun without specifics).
    ///
    /// Flags subjects like "update code", "improve things", "change functionality"
    /// but allows "update dependency versions", "improve error messages in validator".
    fn is_generic_subject(subject: &str) -> bool {
        const GENERIC_VERBS: &[&str] = &["update", "improve", "change", "modify", "enhance"];
        const GENERIC_NOUNS: &[&str] = &[
            "code",
            "things",
            "stuff",
            "functionality",
            "logic",
            "implementation",
            "behavior",
            "performance",
            "handling",
            "processing",
        ];

        let words: Vec<&str> = subject.split_whitespace().collect();
        if words.len() > 4 {
            // Longer subjects are usually specific enough
            return false;
        }

        let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();

        // Check if starts with generic verb and contains a generic noun
        if let Some(first) = lower.first()
            && GENERIC_VERBS.contains(&first.as_str())
        {
            return lower[1..]
                .iter()
                .any(|w| GENERIC_NOUNS.contains(&w.as_str()));
        }

        false
    }

    /// Format violations as a CORRECTIONS section for a retry prompt.
    #[must_use]
    pub fn format_corrections(violations: &[String]) -> String {
        let mut section =
            String::from("\nCORRECTIONS (your previous output had these errors — fix them):\n");
        for v in violations {
            section.push_str("- ");
            section.push_str(v);
            section.push('\n');
        }
        section
    }
}
