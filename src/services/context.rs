use crate::domain::{
    ChangeStatus, CodeSymbol, CommitType, FileCategory, PromptContext, StagedChanges, SymbolKind,
};

/// Token budget constants (in characters, ~4 chars per token)
/// Target: qwen3:4b has ~8K context, we use ~6K to be safe
const MAX_CONTEXT_CHARS: usize = 24_000;
const SYSTEM_PROMPT_RESERVE: usize = 2_000;
const MIN_DIFF_BUDGET: usize = 8_000;

pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build(
        changes: &StagedChanges,
        symbols: &[CodeSymbol],
        max_diff_lines: usize,
    ) -> PromptContext {
        let commit_type = Self::infer_commit_type(changes, symbols);
        let scope = Self::infer_scope(changes);

        // Build components with budget management
        let change_summary = Self::summarize_changes(changes);
        let file_breakdown = Self::format_files(changes);

        // Calculate remaining budget for symbols and diff
        let used = SYSTEM_PROMPT_RESERVE + change_summary.len() + file_breakdown.len();
        let remaining = MAX_CONTEXT_CHARS.saturating_sub(used);

        // Symbols get 20% of remaining, diff gets 80% (minimum MIN_DIFF_BUDGET)
        let diff_budget = remaining.saturating_sub(remaining / 5).max(MIN_DIFF_BUDGET);
        let symbol_budget = remaining.saturating_sub(diff_budget);

        let symbols_added = Self::format_symbols_with_budget(symbols, true, symbol_budget / 2);
        let symbols_removed = Self::format_symbols_with_budget(symbols, false, symbol_budget / 2);

        // Diff gets remaining budget
        let actual_diff_budget = MAX_CONTEXT_CHARS
            .saturating_sub(used)
            .saturating_sub(symbols_added.len())
            .saturating_sub(symbols_removed.len());

        let truncated_diff =
            Self::truncate_diff_with_budget(changes, max_diff_lines, actual_diff_budget);

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

    fn truncate_diff_with_budget(
        changes: &StagedChanges,
        max_lines: usize,
        char_budget: usize,
    ) -> String {
        let mut output = String::new();
        let mut remaining_lines = max_lines;
        let mut files_included = 0;
        let total_files = changes.files.len();

        for file in changes.files_by_priority() {
            if remaining_lines == 0 || file.is_binary {
                continue;
            }

            // Check character budget
            if output.len() >= char_budget {
                break;
            }

            let header = format!("\n--- {} ---\n", file.path.display());

            // Estimate if we have room for at least some content
            if output.len() + header.len() + 100 > char_budget {
                break;
            }

            output.push_str(&header);
            files_included += 1;

            let lines: Vec<_> = file.diff.lines().collect();
            let take = lines.len().min(remaining_lines);

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

            remaining_lines = remaining_lines.saturating_sub(take);
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
