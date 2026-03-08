// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

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
    "Pipfile.lock",
    "uv.lock",
    "pubspec.lock",
    "flake.lock",
    "shrinkwrap.yaml",
    "mix.lock",
];

pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build(
        changes: &StagedChanges,
        symbols: &[CodeSymbol],
        config: &Config,
    ) -> PromptContext {
        let commit_type = Self::infer_commit_type(changes, symbols);
        let scope = if config.format.include_scope {
            Self::infer_scope(changes)
        } else {
            None
        };

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

        // Tri-state symbol classification:
        // - AddedOnly: symbol appears only in added set
        // - RemovedOnly: symbol appears only in removed set
        // - Modified: same name+kind+file in both added and removed (signature changed)
        //
        // Modified symbols are shown separately (not as both Added and Removed, which
        // misleads the LLM). Public modified symbols contribute to breaking risk.
        let modified_symbols: Vec<&CodeSymbol> = symbols
            .iter()
            .filter(|s| {
                s.is_added
                    && symbols.iter().any(|other| {
                        !other.is_added
                            && other.kind == s.kind
                            && other.name == s.name
                            && other.file == s.file
                    })
            })
            .collect();

        let symbols_deduped: Vec<CodeSymbol> = symbols
            .iter()
            .filter(|s| {
                !symbols.iter().any(|other| {
                    other.is_added != s.is_added
                        && other.kind == s.kind
                        && other.name == s.name
                        && other.file == s.file
                })
            })
            .cloned()
            .collect();

        let symbols_added =
            Self::format_symbols_with_budget(&symbols_deduped, true, symbol_budget / 3);
        let symbols_removed =
            Self::format_symbols_with_budget(&symbols_deduped, false, symbol_budget / 3);

        // Format modified symbols (signature changes)
        let symbols_modified = Self::format_modified_symbols(&modified_symbols, symbol_budget / 3);

        // Highlight removed public symbols — strong signal for breaking changes
        let public_api_removed = Self::format_public_api_removed(&symbols_deduped);

        // Diff gets remaining budget
        let actual_diff_budget = max_context
            .saturating_sub(used)
            .saturating_sub(symbols_added.len())
            .saturating_sub(symbols_removed.len())
            .saturating_sub(symbols_modified.len())
            .saturating_sub(public_api_removed.len());

        let truncated_diff = Self::truncate_diff_adaptive(changes, config, actual_diff_budget);

        // Evidence flags for constraint-based anti-hallucination
        let is_mechanical = Self::detect_mechanical_transform(changes, &symbols_deduped);
        let has_bug_evidence = Self::detect_bug_evidence(changes);
        // RemovedOnly public symbols + modified public symbols both contribute to breaking risk
        let public_api_removed_count = symbols_deduped
            .iter()
            .filter(|s| !s.is_added && s.is_public)
            .count()
            + modified_symbols.iter().filter(|s| s.is_public).count();
        let has_new_public_api = symbols_deduped.iter().any(|s| s.is_added && s.is_public);
        let is_dependency_only = Self::detect_dependency_only(changes);

        PromptContext {
            change_summary,
            file_breakdown,
            symbols_added,
            symbols_removed,
            symbols_modified,
            public_api_removed,
            suggested_type: commit_type,
            suggested_scope: scope,
            truncated_diff,
            is_mechanical,
            has_bug_evidence,
            public_api_removed_count,
            has_new_public_api,
            is_dependency_only,
            file_count: changes.files.len(),
            primary_change: Self::detect_primary_change(changes, &symbols_deduped),
            group_rationale: None, // Set by splitter when generating per-group prompts
            metadata_breaking_signals: Self::detect_metadata_breaking(changes),
        }
    }

    pub fn infer_commit_type(changes: &StagedChanges, symbols: &[CodeSymbol]) -> CommitType {
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

        // Explicit bug evidence -> fix
        if Self::detect_bug_evidence(changes) {
            return CommitType::Fix;
        }

        // New public functions/structs -> feat (unless it's an API replacement)
        let has_new_public_symbols = symbols.iter().any(|s| {
            s.is_added
                && s.is_public
                && matches!(
                    s.kind,
                    SymbolKind::Function | SymbolKind::Struct | SymbolKind::Trait
                )
        });

        let has_removed_public_symbols = symbols.iter().any(|s| {
            !s.is_added
                && s.is_public
                && matches!(
                    s.kind,
                    SymbolKind::Function | SymbolKind::Struct | SymbolKind::Trait
                )
        });

        // API replacement: adding new public APIs while removing old ones → refactor
        if has_new_public_symbols && has_removed_public_symbols {
            return CommitType::Refactor;
        }

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

        // Balanced small changes (additions ≈ deletions) with no new symbols -> style/refactor
        // This catches mechanical transformations like reformatting, collapsing nesting, etc.
        if changes.stats.insertions < 20 && changes.stats.deletions < 20 {
            let has_any_new_symbols = symbols.iter().any(|s| s.is_added);
            let has_any_removed_symbols = symbols.iter().any(|s| !s.is_added);
            let balanced = changes.stats.insertions.abs_diff(changes.stats.deletions) <= 5;

            if balanced && !has_any_new_symbols && !has_any_removed_symbols {
                return CommitType::Style;
            }

            return CommitType::Refactor;
        }

        CommitType::Refactor
    }

    pub fn infer_scope(changes: &StagedChanges) -> Option<String> {
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
                "src" | "lib" | "app" | "internal" | "cmd" | "api" | "modules" => {
                    if let Some(next) = components.get(i + 1)
                        && !next.contains('.')
                        && *next != "main"
                        && *next != "lib"
                        && *next != "mod"
                        && *next != "index"
                    {
                        return Some(next.to_string());
                    }
                }
                "packages" | "crates" | "apps" | "services" | "plugins" | "workspaces" => {
                    if let Some(next) = components.get(i + 1)
                        && !next.contains('.')
                    {
                        return Some(next.to_string());
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

    /// Format modified symbols (signature changed: same name+kind+file in both added and removed).
    fn format_modified_symbols(symbols: &[&CodeSymbol], char_budget: usize) -> String {
        if symbols.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        let mut count = 0;

        for sym in symbols {
            let visibility = if sym.is_public { "pub " } else { "" };
            let line = format!(
                "[~] {}{:?} {} ({}:{})",
                visibility,
                sym.kind,
                sym.name,
                sym.file.display(),
                sym.line
            );
            if output.len() + line.len() + 1 > char_budget {
                break;
            }
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&line);
            count += 1;
        }

        let remaining = symbols.len() - count;
        if remaining > 0 {
            output.push_str(&format!("\n... and {} more modified symbols", remaining));
        }

        output
    }

    /// Format removed public symbols as a prominent warning for the LLM.
    /// This helps small models detect breaking changes they would otherwise miss.
    fn format_public_api_removed(symbols: &[CodeSymbol]) -> String {
        let removed_public: Vec<_> = symbols
            .iter()
            .filter(|s| !s.is_added && s.is_public)
            .collect();

        if removed_public.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        for symbol in &removed_public {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&symbol.to_string());
        }
        output
    }

    /// Detect if changes are a mechanical/syntactic transformation with no semantic impact.
    ///
    /// True when: no symbol definitions changed, adds ≈ removes (balanced),
    /// and changes are small. Catches reformatting, nesting collapse, import reorder.
    fn detect_mechanical_transform(changes: &StagedChanges, symbols: &[CodeSymbol]) -> bool {
        // Any symbol added or removed means it's not purely mechanical
        if !symbols.is_empty() {
            return false;
        }

        let ins = changes.stats.insertions;
        let del = changes.stats.deletions;
        let total = ins + del;

        // Need actual changes, and they should be small
        if total == 0 || total > 80 {
            return false;
        }

        // Must be balanced (adds ≈ removes)
        let balance = ins.min(del) as f64 / ins.max(del).max(1) as f64;
        balance > 0.5
    }

    /// Detect if the diff contains evidence of a bug fix.
    ///
    /// Conservative: only flags explicit fix/bug comments in added lines.
    /// When false, the model is guided away from using "fix" type.
    fn detect_bug_evidence(changes: &StagedChanges) -> bool {
        changes.files.iter().any(|f| {
            f.diff
                .lines()
                .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                .any(|l| {
                    let lower = l[1..].to_lowercase();
                    lower.contains("// fix")
                        || lower.contains("# fix")
                        || lower.contains("/* fix")
                        || lower.contains("// bug")
                        || lower.contains("# bug")
                        || lower.contains("fixme")
                        || lower.contains("hotfix")
                })
        })
    }

    /// Detect if all changes are to dependency/config files only.
    fn detect_dependency_only(changes: &StagedChanges) -> bool {
        !changes.files.is_empty()
            && changes
                .files
                .iter()
                .all(|f| matches!(f.category, FileCategory::Config | FileCategory::Build))
    }

    /// Identify the most significant change to anchor the subject line.
    ///
    /// Priority: new public APIs > removed public APIs > largest file by change size > new private symbols.
    fn detect_primary_change(changes: &StagedChanges, symbols: &[CodeSymbol]) -> Option<String> {
        // 1. New public API (highest signal)
        let new_public: Vec<_> = symbols
            .iter()
            .filter(|s| s.is_added && s.is_public)
            .collect();
        if let Some(sym) = new_public.first() {
            return Some(format!("added {:?} {} (public)", sym.kind, sym.name));
        }

        // 2. Removed public API (breaking = important)
        let removed_public: Vec<_> = symbols
            .iter()
            .filter(|s| !s.is_added && s.is_public)
            .collect();
        if let Some(sym) = removed_public.first() {
            return Some(format!("removed {:?} {} (public)", sym.kind, sym.name));
        }

        // 3. File with most lines changed
        let biggest = changes
            .files
            .iter()
            .max_by_key(|f| f.additions + f.deletions);
        if let Some(f) = biggest {
            let stem = f
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            return Some(format!(
                "most changes in {} (+{} -{})",
                stem, f.additions, f.deletions
            ));
        }

        None
    }

    /// Scan diff content for metadata changes that indicate breaking changes.
    ///
    /// Detects: MSRV bumps, minimum engine/runtime version raises, removed features/exports.
    fn detect_metadata_breaking(changes: &StagedChanges) -> Vec<String> {
        let mut signals = Vec::new();

        for file in &changes.files {
            let name = file.path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            for line in file.diff.lines() {
                let is_removed = line.starts_with('-') && !line.starts_with("---");
                let is_added = line.starts_with('+') && !line.starts_with("+++");
                let content = if is_removed || is_added {
                    &line[1..]
                } else {
                    continue;
                };

                match name {
                    "Cargo.toml" => {
                        // rust-version (MSRV) changed
                        if content.contains("rust-version") && is_added {
                            signals.push(format!("MSRV changed in Cargo.toml: {}", content.trim()));
                        }
                    }
                    "package.json" => {
                        // engines.node minimum raised
                        if content.contains("\"node\"") && is_added && content.contains("engines") {
                            signals.push(format!(
                                "Node engine requirement changed: {}",
                                content.trim()
                            ));
                        }
                    }
                    "pyproject.toml" => {
                        // requires-python minimum raised
                        if content.contains("requires-python") && is_added {
                            signals.push(format!(
                                "Python version requirement changed: {}",
                                content.trim()
                            ));
                        }
                    }
                    _ => {}
                }

                // Cross-file: removed feature flags
                if is_removed
                    && name == "Cargo.toml"
                    && content.trim_start().starts_with('[')
                    && content.contains("features")
                {
                    signals.push("Cargo.toml [features] section modified".to_string());
                }

                // Removed public exports
                if is_removed {
                    let trimmed = content.trim();
                    if trimmed.starts_with("pub use ") || trimmed.starts_with("pub mod ") {
                        signals.push(format!("Removed public re-export: {}", trimmed));
                    }
                    if trimmed.starts_with("export {") || trimmed.starts_with("export default") {
                        signals.push(format!("Removed JS/TS export: {}", trimmed));
                    }
                }
            }
        }

        signals.dedup();
        signals
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
    fn truncate_diff_adaptive(
        changes: &StagedChanges,
        config: &Config,
        char_budget: usize,
    ) -> String {
        let mut output = String::with_capacity(char_budget);
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
