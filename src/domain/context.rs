// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use super::CommitType;

#[derive(Debug)]
pub struct PromptContext {
    pub change_summary: String,
    pub file_breakdown: String,
    pub symbols_added: String,
    pub symbols_removed: String,
    pub suggested_type: CommitType,
    pub suggested_scope: Option<String>,
    pub truncated_diff: String,
}

impl PromptContext {
    pub fn to_prompt(&self) -> String {
        let symbols_section = self.format_symbols_section();

        format!(
            r#"Analyze this git diff and generate a commit message.

SUMMARY: {summary}
FILES: {files}
SUGGESTED TYPE: {commit_type}{scope}
{symbols}
DIFF:
{diff}

Write a JSON commit message describing the changes shown in the diff.
The subject must be specific - describe WHAT was changed (e.g., "add system prompt to ollama provider", "update dependency versions").

Output format:
{{"type": "{commit_type}", "scope": {scope_json}, "subject": "<your description here>", "body": null}}"#,
            summary = self.change_summary,
            files = self.file_breakdown.trim(),
            commit_type = self.suggested_type.as_str(),
            scope = self
                .suggested_scope
                .as_ref()
                .map(|s| format!("\nSCOPE: {}", s))
                .unwrap_or_default(),
            symbols = symbols_section,
            scope_json = self
                .suggested_scope
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or_else(|| "null".to_string()),
            diff = self.truncated_diff,
        )
    }

    fn format_symbols_section(&self) -> String {
        let has_added = !self.symbols_added.is_empty();
        let has_removed = !self.symbols_removed.is_empty();

        if !has_added && !has_removed {
            return String::new();
        }

        let mut section = String::from("\nSYMBOLS CHANGED:");
        if has_added {
            section.push_str(&format!(
                "\n  Added:\n    {}",
                self.symbols_added.replace('\n', "\n    ")
            ));
        }
        if has_removed {
            section.push_str(&format!(
                "\n  Removed:\n    {}",
                self.symbols_removed.replace('\n', "\n    ")
            ));
        }
        section.push('\n');
        section
    }
}
