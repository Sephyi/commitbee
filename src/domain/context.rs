// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use super::CommitType;

#[derive(Debug)]
pub struct PromptContext {
    pub change_summary: String,
    pub file_breakdown: String,
    pub symbols_added: String,
    pub symbols_removed: String,
    pub symbols_modified: String,
    pub public_api_removed: String,
    pub suggested_type: CommitType,
    pub suggested_scope: Option<String>,
    pub truncated_diff: String,
    // Evidence flags for constraint-based anti-hallucination
    pub is_mechanical: bool,
    pub has_bug_evidence: bool,
    pub public_api_removed_count: usize,
    pub has_new_public_api: bool,
    pub is_dependency_only: bool,
    /// Number of files in this group (for focus instruction on large groups)
    pub file_count: usize,
    /// Most significant change for subject anchoring (e.g., "added CommitValidator struct")
    pub primary_change: Option<String>,
    /// Short rationale for why these files were grouped together
    pub group_rationale: Option<String>,
    /// Metadata-level breaking signals detected from diff content
    pub metadata_breaking_signals: Vec<String>,
    /// ISO 639-1 locale code for non-English commit messages (e.g., "de", "ja")
    pub locale: Option<String>,
    /// Optional project style context learned from commit history
    pub history_context: Option<String>,
    /// Cross-symbol relationships detected from diff content.
    /// e.g., "validate calls parse() — both changed"
    pub connections: Vec<String>,
    /// Import/use statement changes detected from diff.
    /// e.g., "analyzer: added use crate::domain::DiffHunk"
    pub import_changes: Vec<String>,
    /// Source-to-test file correlations detected from staged changes.
    /// e.g., "src/services/context.rs <-> tests/context.rs (test file)"
    pub test_correlations: Vec<String>,
}

impl PromptContext {
    #[must_use]
    pub fn to_prompt(&self) -> String {
        let symbols_section = self.format_symbols_section();
        let breaking_warning = self.format_breaking_warning();
        let evidence_section = self.format_evidence_section();
        let constraints_section = self.format_constraints_section();
        let connections_section = if self.connections.is_empty() {
            String::new()
        } else {
            format!(
                "\nCONNECTIONS:\n{}\n",
                self.connections
                    .iter()
                    .map(|c| format!("  {}", c))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        let imports_section = if self.import_changes.is_empty() {
            String::new()
        } else {
            format!(
                "\nIMPORTS CHANGED:\n{}\n",
                self.import_changes
                    .iter()
                    .map(|i| format!("  {}", i))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        let related_section = if self.test_correlations.is_empty() {
            String::new()
        } else {
            format!(
                "\nRELATED FILES:\n{}\n",
                self.test_correlations
                    .iter()
                    .map(|c| format!("  {}", c))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        // Calculate available chars for subject after type(scope)[!]: prefix
        let prefix_len = self.suggested_type.as_str().len()
            + self
                .suggested_scope
                .as_ref()
                .map(|s| s.len() + 2)
                .unwrap_or(0) // "(scope)"
            + if self.public_api_removed_count > 0 { 1 } else { 0 } // "!" for breaking
            + 2; // ": "
        let subject_budget = 72_usize.saturating_sub(prefix_len);

        let focus_instruction = if self.file_count > 5 {
            "\nFOCUS: This group contains many files. Focus the subject on the single most significant change. Do not try to describe every change — pick the primary one.\n"
        } else {
            ""
        };

        let primary_change_line = self
            .primary_change
            .as_ref()
            .map(|pc| format!("\nPRIMARY_CHANGE: {}\n", pc))
            .unwrap_or_default();

        let group_rationale_line = self
            .group_rationale
            .as_ref()
            .map(|gr| format!("GROUP_REASON: {}\n", gr))
            .unwrap_or_default();

        let locale_instruction = self
            .locale
            .as_ref()
            .map(|lang| {
                format!(
                    "\nLANGUAGE: Write the subject and body in {lang}. \
                     The commit type, scope, and JSON keys must remain in English.\n"
                )
            })
            .unwrap_or_default();

        let history_section = self
            .history_context
            .as_ref()
            .map(|h| format!("\n{}\n", h))
            .unwrap_or_default();

        let metadata_breaking_section = if self.metadata_breaking_signals.is_empty() {
            String::new()
        } else {
            format!(
                "\nWARNING: METADATA BREAKING CHANGES DETECTED:\n{}\n",
                self.metadata_breaking_signals
                    .iter()
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        format!(
            r#"Analyze this git diff and generate a commit message.

SUMMARY: {summary}
FILES: {files}
SUGGESTED TYPE: {commit_type}{scope}
{group_rationale}{evidence}{primary_change}{symbols}{connections}{imports}{related}
DIFF:
{diff}
{constraints}{breaking}{metadata_breaking}{locale}{focus}{history}
HARD LIMIT: subject must be under {subject_budget} chars (count carefully). Name at least one concrete entity (function, struct, variable) from the diff.
Body: 1-3 sentences on WHY, or null if trivial. breaking_change: only if existing users must change code/config to stay compatible, else null.

Respond with ONLY this JSON:
{{"type": "<type>", "scope": {scope_json}, "subject": "<MUST be under {subject_budget} chars>", "body": null, "breaking_change": null}}"#,
            summary = self.change_summary,
            files = self.file_breakdown.trim(),
            commit_type = self.suggested_type.as_str(),
            scope = self
                .suggested_scope
                .as_ref()
                .map(|s| format!("\nSCOPE: {}", s))
                .unwrap_or_default(),
            evidence = evidence_section,
            symbols = symbols_section,
            connections = connections_section,
            imports = imports_section,
            related = related_section,
            breaking = breaking_warning,
            constraints = constraints_section,
            focus = focus_instruction,
            primary_change = primary_change_line,
            group_rationale = group_rationale_line,
            metadata_breaking = metadata_breaking_section,
            locale = locale_instruction,
            history = history_section,
            subject_budget = subject_budget,
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
        let has_modified = !self.symbols_modified.is_empty();

        if !has_added && !has_removed && !has_modified {
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
        if has_modified {
            section.push_str(&format!(
                "\n  Modified (signature changed):\n    {}",
                self.symbols_modified.replace('\n', "\n    ")
            ));
        }
        section.push('\n');
        section
    }

    fn format_breaking_warning(&self) -> String {
        if self.public_api_removed.is_empty() {
            return String::new();
        }

        format!(
            "\nWARNING: PUBLIC API REMOVED — describe this in breaking_change field:\n    {}\n",
            self.public_api_removed.replace('\n', "\n    ")
        )
    }

    fn format_evidence_section(&self) -> String {
        // Skip when all flags are at default — saves ~200 chars for small models
        if !self.is_mechanical
            && !self.has_bug_evidence
            && self.public_api_removed_count == 0
            && !self.has_new_public_api
            && !self.is_dependency_only
        {
            return String::new();
        }

        let yn = |b: bool| if b { "yes" } else { "no" };

        format!(
            "\nEVIDENCE:\n\
             - Is this a mechanical/formatting change? {}\n\
             - Does the diff contain bug-fix comments? {}\n\
             - How many public APIs were removed? {}\n\
             - Were new public APIs added? {}\n\
             - Are all changes in dependency/config files? {}\n",
            yn(self.is_mechanical),
            yn(self.has_bug_evidence),
            self.public_api_removed_count,
            yn(self.has_new_public_api),
            yn(self.is_dependency_only),
        )
    }

    fn format_constraints_section(&self) -> String {
        let mut rules = Vec::new();

        if !self.has_bug_evidence {
            rules.push(
                "- No bug-fix comments found: prefer \"refactor\" over \"fix\". \
                 Only use \"fix\" if the diff clearly corrects wrong behavior.",
            );
        }
        if self.is_mechanical {
            rules.push(
                "- Mechanical/formatting change detected: use \"style\" or \"refactor\", not \"feat\" or \"fix\".",
            );
        }
        if self.public_api_removed_count > 0 {
            rules.push(
                "- Public APIs were removed: set breaking_change to describe what was removed \
                 (e.g., \"removed `old_fn()`, use `new_fn()` instead\"). \
                 Never copy labels from this prompt as the description.",
            );
        }
        if self.is_dependency_only {
            rules.push("- All changes are in dependency/config files: use \"chore\".");
        }
        if !self.metadata_breaking_signals.is_empty() {
            rules.push(
                "- Metadata breaking changes detected (version requirements raised, features removed, etc.): \
                 set breaking_change to describe the compatibility impact.",
            );
        }

        if rules.is_empty() {
            return String::new();
        }

        format!("\nCONSTRAINTS (must follow):\n{}\n", rules.join("\n"))
    }
}
