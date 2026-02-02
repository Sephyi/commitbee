// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
// SPDX-License-Identifier: GPL-3.0-only

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
        format!(
            r#"Analyze this git diff and generate a commit message.

SUMMARY: {summary}
FILES: {files}
SUGGESTED TYPE: {commit_type}{scope}

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
            scope_json = self
                .suggested_scope
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or_else(|| "null".to_string()),
            diff = self.truncated_diff,
        )
    }
}
