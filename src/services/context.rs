// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use crate::domain::{
    ChangeStatus, CodeSymbol, CommitType, FileCategory, PromptContext, StagedChanges, SymbolKind,
};

const SYSTEM_PROMPT_RESERVE: usize = 2_000;
const MIN_DIFF_BUDGET: usize = 4_000;

/// Lock files to skip content for (just show that they changed)
const SKIP_CONTENT_FILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lockb",
    "go.sum",
    "Gemfile.lock",
    "poetry.lock",
    "composer.lock",
];

pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build(changes: &StagedChanges, symbols: &[CodeSymbol], config: &Config) -> PromptContext {
        let commit_type = Self::infer_commit_type(changes, symbols);
        let scope = Self::infer_scope(changes);

        // Build components with budget management
        let change_summary = Self::summarize_changes(changes);
        let file_breakdown = Self::format_files(changes);

        // Calculate remaining budget for symbols and diff
        let max_context = config.max_context_chars;
        let used = SYSTEM_PROMPT_RESERVE + change_summary.len() + file_breakdown.len();
        let remaining = max_context.saturating_sub(used);

        // Symbols get 20% of remaining, diff gets 80% (minimum MIN_DIFF_BUDGET)
        let diff_budget = remaining.saturating_sub(remaining / 5).max(MIN_DIFF_BUDGET);
        let symbol_budget = remaining.saturating_sub(diff_budget);

        let symbols_added = Self::format_symbols_with_budget(symbols, true, symbol_budget / 2);
        let symbols_removed = Self::format_symbols_with_budget(symbols, false, symbol_budget / 2);

        // Diff gets remaining budget
        let actual_diff_budget = max_context
            .saturating_sub(used)
            .saturating_sub(symbols_added.len())
            .saturating_sub(symbols_removed.len());

        let truncated_diff = Self::truncate_diff_adaptive(changes, config, actual_diff_budget);

        PromptContext {
            change_summary,
            file_breakdown,
            symbols_added,
            symbols_removed,
            suggested_type: commit_type,
            suggested_scope: scope,
            truncated_diff,
        }
    }

    fn infer_commit_type(changes: &StagedChanges, symbols: &[CodeSymbol]) -> CommitType {
        let categories: Vec<_> = changes.files.iter().map(|f| f.category).collect();

        // All docs -> docs
        if categories.iter().all(|c| *c == FileCategory::Docs) {
            return CommitType::Docs;
        }

        // All tests -> test
        if categories.iter().all(|c| *c == FileCategory::Test) {
            return CommitType::Test;
        }

        // All config -> chore
        if categories.iter().all(|c| *c == FileCategory::Config) {
            return CommitType::Chore;
        }

        // All build -> build
        if categories.iter().all(|c| *c == FileCategory::Build) {
            return CommitType::Build;
        }

        // New public functions/structs -> feat
        let has_new_public_symbols = symbols.iter().any(|s| {
            s.is_added
                && s.is_public
                && matches!(
                    s.kind,
                    SymbolKind::Function | SymbolKind::Struct | SymbolKind::Trait
                )
        });

        if has_new_public_symbols {
            return CommitType::Feat;
        }

        // New files dominate -> feat
        let new_file_count = changes
            .files
            .iter()
            .filter(|f| f.status == ChangeStatus::Added)
            .count();

        if new_file_count > changes.files.len() / 2 {
            return CommitType::Feat;
        }

        // More deletions than additions -> refactor
        if changes.stats.deletions > changes.stats.insertions * 2 {
            return CommitType::Refactor;
        }

        // Small changes -> fix
        if changes.stats.insertions < 20 && changes.stats.deletions < 20 {
            return CommitType::Fix;
        }

        CommitType::Feat
    }

    fn infer_scope(changes: &StagedChanges) -> Option<String> {
        let scopes: Vec<_> = changes
            .files
            .iter()
            .filter(|f| f.category == FileCategory::Source)
            .filter_map(|f| Self::extract_scope_from_path(&f.path))
            .collect();

        if scopes.is_empty() {
            return None;
        }

        // If all same scope
        let first = &scopes[0];
        if scopes.iter().all(|s| s == first) {
            return Some(first.clone());
        }

        None
    }

    fn extract_scope_from_path(path: &std::path::Path) -> Option<String> {
        let components: Vec<_> = path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        for (i, component) in components.iter().enumerate() {
            match *component {
                "src" | "lib" => {
                    if let Some(next) = components.get(i + 1) {
                        if !next.contains('.')
                            && *next != "main"
                            && *next != "lib"
                            && *next != "mod"
                        {
                            return Some(next.to_string());
                        }
                    }
                }
                "packages" | "crates" | "apps" => {
                    if let Some(next) = components.get(i + 1) {
                        if !next.contains('.') {
                            return Some(next.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .filter(|n| !matches!(*n, "src" | "lib" | "." | ""))
            .map(|s| s.to_string())
    }

    fn summarize_changes(changes: &StagedChanges) -> String {
        let added = changes
            .files
            .iter()
            .filter(|f| f.status == ChangeStatus::Added)
            .count();
        let modified = changes
            .files
            .iter()
            .filter(|f| f.status == ChangeStatus::Modified)
            .count();
        let deleted = changes
            .files
            .iter()
            .filter(|f| f.status == ChangeStatus::Deleted)
            .count();

        format!(
            "{} files ({} added, {} modified, {} deleted) | +{} -{}",
            changes.files.len(),
            added,
            modified,
            deleted,
            changes.stats.insertions,
            changes.stats.deletions
        )
    }

    fn format_files(changes: &StagedChanges) -> String {
        let mut output = String::new();

        for file in changes.files_by_priority() {
            if file.is_binary {
                continue;
            }

            let status = match file.status {
                ChangeStatus::Added => "[+]",
                ChangeStatus::Modified => "[M]",
                ChangeStatus::Deleted => "[-]",
                ChangeStatus::Renamed => "[R]",
            };

            output.push_str(&format!(
                "{} {} (+{} -{})\n",
                status,
                file.path.display(),
                file.additions,
                file.deletions
            ));
        }

        output
    }

    fn format_symbols_with_budget(
        symbols: &[CodeSymbol],
        added: bool,
        char_budget: usize,
    ) -> String {
        let filtered: Vec<_> = symbols.iter().filter(|s| s.is_added == added).collect();

        if filtered.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        let mut count = 0;

        for symbol in &filtered {
            let line = symbol.to_string();
            if output.len() + line.len() + 1 > char_budget {
                break;
            }
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&line);
            count += 1;
        }

        // Indicate if we truncated
        let remaining = filtered.len() - count;
        if remaining > 0 {
            output.push_str(&format!("\n... and {} more symbols", remaining));
        }

        output
    }

    /// Check if a file should have its content skipped (lock files, etc.)
    fn should_skip_content(path: &std::path::Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        SKIP_CONTENT_FILES.contains(&name)
    }

    /// Calculate adaptive per-file line budget based on file count and priority
    fn calculate_file_budget(
        file_count: usize,
        category: FileCategory,
        max_diff_lines: usize,
    ) -> usize {
        // Weight by category: source gets more, config/lock gets less
        let weight = match category {
            FileCategory::Source => 3,
            FileCategory::Test => 2,
            FileCategory::Docs => 1,
            FileCategory::Config => 1,
            FileCategory::Build => 1,
            FileCategory::Other => 1,
        };

        // Base budget per file, adjusted by count
        let base_per_file = match file_count {
            1 => max_diff_lines,                        // Single file gets full budget
            2..=3 => max_diff_lines / 2,                // 2-3 files: split evenly
            4..=6 => max_diff_lines / file_count,       // 4-6 files: proportional
            _ => (max_diff_lines / file_count).max(30), // Many files: minimum 30 lines
        };

        // Apply category weight (source files get more)
        (base_per_file * weight / 2).max(20)
    }

    /// Adaptive diff truncation: smarter budget allocation per file
    fn truncate_diff_adaptive(changes: &StagedChanges, config: &Config, char_budget: usize) -> String {
        let mut output = String::new();
        let mut files_included = 0;
        let total_files = changes.files.len();
        let files = changes.files_by_priority();

        // Count non-binary, non-skip files for budget calculation
        let content_files: Vec<_> = files
            .iter()
            .filter(|f| !f.is_binary && !Self::should_skip_content(&f.path))
            .collect();

        for file in &files {
            if file.is_binary {
                continue;
            }

            // Check character budget
            if output.len() >= char_budget {
                break;
            }

            let header = format!("\n--- {} ---\n", file.path.display());

            // Estimate if we have room for at least some content
            if output.len() + header.len() + 50 > char_budget {
                break;
            }

            output.push_str(&header);
            files_included += 1;

            // Skip content for lock files
            if Self::should_skip_content(&file.path) {
                output.push_str("(lock file - content skipped)\n");
                continue;
            }

            // Calculate adaptive line budget for this file
            let file_line_budget = Self::calculate_file_budget(
                content_files.len(),
                file.category,
                config.max_diff_lines,
            )
            .min(config.max_file_lines);

            let lines: Vec<_> = file.diff.lines().collect();
            let take = lines.len().min(file_line_budget);

            for line in &lines[..take] {
                // Check char budget before each line
                if output.len() + line.len() + 1 > char_budget {
                    output.push_str("... (budget exceeded)\n");
                    break;
                }
                output.push_str(line);
                output.push('\n');
            }

            if lines.len() > take {
                output.push_str(&format!("... ({} lines truncated)\n", lines.len() - take));
            }
        }

        // Indicate if files were skipped
        let skipped = total_files - files_included;
        if skipped > 0 {
            output.push_str(&format!(
                "\n... ({} files not shown due to budget)\n",
                skipped
            ));
        }

        output
    }
}
