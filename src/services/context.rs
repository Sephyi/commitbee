// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::collections::HashSet;

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
        // Build components with budget management
        let change_summary = Self::summarize_changes(changes);
        let file_breakdown = Self::format_files(changes);

        // Calculate remaining budget for symbols and diff
        let max_context = config.max_context_chars;
        let used = SYSTEM_PROMPT_RESERVE + change_summary.len() + file_breakdown.len();
        let remaining = max_context.saturating_sub(used);

        // Symbols get 30% when signatures are present (richer content), 20% otherwise
        let symbol_pct = if symbols.iter().any(|s| s.signature.is_some()) {
            30
        } else {
            20
        };
        let diff_budget = remaining
            .saturating_sub(remaining * symbol_pct / 100)
            .max(MIN_DIFF_BUDGET);
        let symbol_budget = remaining.saturating_sub(diff_budget);

        // Tri-state symbol classification:
        // - AddedOnly: symbol appears only in added set
        // - RemovedOnly: symbol appears only in removed set
        // - Modified: same name+kind+file in both added and removed (signature changed)
        //
        // Modified symbols are shown separately (not as both Added and Removed, which
        // misleads the LLM). Public modified symbols contribute to breaking risk.
        //
        // Uses HashSet for O(1) lookup instead of O(N^2) nested iteration (P2).
        type SymbolKey<'a> = (&'a SymbolKind, &'a str, &'a std::path::Path);

        let added_keys: HashSet<SymbolKey<'_>> = symbols
            .iter()
            .filter(|s| s.is_added)
            .map(|s| (&s.kind, s.name.as_str(), s.file.as_path()))
            .collect();

        let removed_keys: HashSet<SymbolKey<'_>> = symbols
            .iter()
            .filter(|s| !s.is_added)
            .map(|s| (&s.kind, s.name.as_str(), s.file.as_path()))
            .collect();

        // Build modified symbols with whitespace classification
        let mut modified_symbols: Vec<CodeSymbol> = symbols
            .iter()
            .filter(|s| {
                s.is_added && removed_keys.contains(&(&s.kind, s.name.as_str(), s.file.as_path()))
            })
            .cloned()
            .collect();

        // Populate is_whitespace_only by comparing diff content within each symbol's span.
        // Uses separate old/new line ranges since the same symbol may be at different
        // line numbers in HEAD vs staged (e.g., lines added above it shift everything).
        for symbol in &mut modified_symbols {
            if let Some(file_change) = changes.files.iter().find(|f| f.path == symbol.file) {
                // Find the old-side counterpart for its line range
                let old_sym = symbols.iter().find(|s| {
                    !s.is_added
                        && s.name == symbol.name
                        && s.kind == symbol.kind
                        && s.file == symbol.file
                });
                let (old_start, old_end) = old_sym
                    .map(|s| (s.line, s.end_line))
                    .unwrap_or((symbol.line, symbol.end_line));

                symbol.is_whitespace_only = Self::classify_span_change(
                    &file_change.diff,
                    symbol.line,
                    symbol.end_line,
                    old_start,
                    old_end,
                );
            }
        }

        let symbols_deduped: Vec<CodeSymbol> = symbols
            .iter()
            .filter(|s| {
                let key: SymbolKey<'_> = (&s.kind, s.name.as_str(), s.file.as_path());
                if s.is_added {
                    !removed_keys.contains(&key)
                } else {
                    !added_keys.contains(&key)
                }
            })
            .cloned()
            .collect();

        // Infer commit type AFTER classification so it can see whitespace-only data
        let all_modified_ws = !modified_symbols.is_empty()
            && modified_symbols
                .iter()
                .all(|s| s.is_whitespace_only == Some(true));
        let commit_type = Self::infer_commit_type(changes, &symbols_deduped, all_modified_ws);
        let scope = if config.format.include_scope {
            Self::infer_scope(changes)
        } else {
            None
        };

        let symbols_added =
            Self::format_symbols_with_budget(&symbols_deduped, true, symbol_budget / 3);
        let symbols_removed =
            Self::format_symbols_with_budget(&symbols_deduped, false, symbol_budget / 3);

        // Collect the removed-side counterparts for modified symbols (to show old→new signatures)
        let modified_old: Vec<&CodeSymbol> = symbols
            .iter()
            .filter(|s| {
                !s.is_added && added_keys.contains(&(&s.kind, s.name.as_str(), s.file.as_path()))
            })
            .collect();

        // Format modified symbols (signature changes), excluding whitespace-only
        let semantic_modified: Vec<&CodeSymbol> = modified_symbols
            .iter()
            .filter(|s| s.is_whitespace_only != Some(true))
            .collect();
        let symbols_modified =
            Self::format_modified_symbols(&semantic_modified, &modified_old, symbol_budget / 3);

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
        // Only genuinely removed public symbols count as "removed API".
        // Modified public symbols (same name in old+new) are NOT removals — their
        // signatures may have changed but the API still exists. Counting them as
        // removed triggers false "breaking_change required" validator violations.
        let public_api_removed_count = symbols_deduped
            .iter()
            .filter(|s| !s.is_added && s.is_public)
            .count();
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
            locale: config.locale.clone(),
            history_context: None, // Set by App when learn_from_history is enabled
            connections: Self::detect_connections(changes, symbols),
            import_changes: Self::detect_import_changes(changes),
        }
    }

    /// Classify whether changes within a symbol span are whitespace-only.
    ///
    /// Tracks old-file and new-file line numbers independently, using separate
    /// spans for each: `new_start..new_end` for added lines, `old_start..old_end`
    /// for removed lines. This correctly handles cases where the same symbol is
    /// at different line numbers in HEAD vs staged (e.g., lines added above it).
    ///
    /// Returns `None` if no changes in span, `Some(true)` if whitespace-only,
    /// `Some(false)` if semantic changes detected.
    pub(crate) fn classify_span_change(
        diff: &str,
        new_start: usize,
        new_end: usize,
        old_start: usize,
        old_end: usize,
    ) -> Option<bool> {
        use crate::services::analyzer::DiffHunk;

        let hunks = DiffHunk::parse_from_diff(diff);
        let mut added_in_span: Vec<&str> = Vec::new();
        let mut removed_in_span: Vec<&str> = Vec::new();

        let mut current_old_line: usize = 0;
        let mut current_new_line: usize = 0;
        let mut hunk_idx: usize = 0;
        let mut in_hunk = false;

        for line in diff.lines() {
            if line.starts_with("@@") {
                if hunk_idx < hunks.len() {
                    current_old_line = hunks[hunk_idx].old_start;
                    current_new_line = hunks[hunk_idx].new_start;
                    hunk_idx += 1;
                    in_hunk = true;
                }
                continue;
            }

            if !in_hunk || line.starts_with("+++") || line.starts_with("---") {
                continue;
            }

            if let Some(content) = line.strip_prefix('+') {
                let in_new_span = current_new_line >= new_start && current_new_line <= new_end;
                if in_new_span {
                    added_in_span.push(content);
                }
                current_new_line += 1;
            } else if let Some(content) = line.strip_prefix('-') {
                let in_old_span = current_old_line >= old_start && current_old_line <= old_end;
                if in_old_span {
                    removed_in_span.push(content);
                }
                current_old_line += 1;
            } else {
                // Context line — advances both counters
                current_old_line += 1;
                current_new_line += 1;
            }
        }

        if added_in_span.is_empty() && removed_in_span.is_empty() {
            return None;
        }

        // Compare non-whitespace character streams.
        // Correctly handles line wrapping while detecting actual content changes.
        let old_text: String = removed_in_span
            .iter()
            .flat_map(|l| l.chars())
            .filter(|c| !c.is_whitespace())
            .collect();
        let new_text: String = added_in_span
            .iter()
            .flat_map(|l| l.chars())
            .filter(|c| !c.is_whitespace())
            .collect();

        Some(old_text == new_text)
    }

    pub fn infer_commit_type(
        changes: &StagedChanges,
        symbols: &[CodeSymbol],
        all_modified_whitespace_only: bool,
    ) -> CommitType {
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

        // All modified symbols are whitespace-only and no added/removed symbols → style
        // (catches `cargo fmt` where symbols exist but only spacing changed)
        if all_modified_whitespace_only && symbols.is_empty() {
            return CommitType::Style;
        }

        // Explicit bug evidence -> fix
        if Self::detect_bug_evidence(changes) {
            return CommitType::Fix;
        }

        // Single-pass symbol evidence: compute all flags in one iteration
        let (has_new_public, has_removed_public, has_any_new, has_any_removed) =
            symbols.iter().fold(
                (false, false, false, false),
                |(mut np, mut rp, mut an, mut ar), s| {
                    let is_api_kind = matches!(
                        s.kind,
                        SymbolKind::Function | SymbolKind::Struct | SymbolKind::Trait
                    );
                    if s.is_added {
                        an = true;
                        if s.is_public && is_api_kind {
                            np = true;
                        }
                    } else {
                        ar = true;
                        if s.is_public && is_api_kind {
                            rp = true;
                        }
                    }
                    (np, rp, an, ar)
                },
            );

        // API replacement: adding new public APIs while removing old ones → refactor
        if has_new_public && has_removed_public {
            return CommitType::Refactor;
        }

        if has_new_public {
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
            let balanced = changes.stats.insertions.abs_diff(changes.stats.deletions) <= 5;

            if balanced && !has_any_new && !has_any_removed {
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
        let renamed = changes
            .files
            .iter()
            .filter(|f| f.status == ChangeStatus::Renamed)
            .count();

        let mut parts = vec![
            format!("{} added", added),
            format!("{} modified", modified),
            format!("{} deleted", deleted),
        ];
        if renamed > 0 {
            parts.push(format!("{} renamed", renamed));
        }

        format!(
            "{} files ({}) | +{} -{}",
            changes.files.len(),
            parts.join(", "),
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

            match file.status {
                ChangeStatus::Renamed => {
                    let old = file
                        .old_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "?".into());
                    let sim = file.rename_similarity.unwrap_or(0);
                    output.push_str(&format!(
                        "[R] {} -> {} ({}% similar, +{} -{})\n",
                        old,
                        file.path.display(),
                        sim,
                        file.additions,
                        file.deletions
                    ));
                }
                _ => {
                    let status = match file.status {
                        ChangeStatus::Added => "[+]",
                        ChangeStatus::Modified => "[M]",
                        ChangeStatus::Deleted => "[-]",
                        ChangeStatus::Renamed => unreachable!(),
                    };
                    output.push_str(&format!(
                        "{} {} (+{} -{})\n",
                        status,
                        file.path.display(),
                        file.additions,
                        file.deletions
                    ));
                }
            }
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
    ///
    /// When both old and new signatures are available and differ, shows the transition as
    /// `[~] old_sig → new_sig (file:line)`. Falls back to signature-only or kind+name display.
    fn format_modified_symbols(
        new_symbols: &[&CodeSymbol],
        old_symbols: &[&CodeSymbol],
        char_budget: usize,
    ) -> String {
        if new_symbols.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        let mut count = 0;

        for new_sym in new_symbols {
            // Match by name+kind+file to support overloaded languages
            let old_sym = old_symbols.iter().find(|s| {
                s.name == new_sym.name && s.kind == new_sym.kind && s.file == new_sym.file
            });

            let line = match (
                old_sym.and_then(|s| s.signature.as_ref()),
                new_sym.signature.as_ref(),
            ) {
                (Some(old_sig), Some(new_sig)) if old_sig != new_sig => {
                    format!(
                        "[~] {} \u{2192} {} ({}:{})",
                        old_sig,
                        new_sig,
                        new_sym.file.display(),
                        new_sym.line
                    )
                }
                (_, Some(sig)) => {
                    format!("[~] {} ({}:{})", sig, new_sym.file.display(), new_sym.line)
                }
                _ => {
                    let visibility = if new_sym.is_public { "pub " } else { "" };
                    format!(
                        "[~] {}{:?} {} ({}:{})",
                        visibility,
                        new_sym.kind,
                        new_sym.name,
                        new_sym.file.display(),
                        new_sym.line
                    )
                }
            };

            if output.len() + line.len() + 1 > char_budget {
                break;
            }
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&line);
            count += 1;
        }

        let remaining = new_symbols.len() - count;
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

    /// Detect cross-file relationships: added lines that call symbols from other changed files.
    fn detect_connections(changes: &StagedChanges, symbols: &[CodeSymbol]) -> Vec<String> {
        let mut connections = Vec::new();

        let symbol_files: Vec<(&str, &std::path::Path)> = symbols
            .iter()
            .filter(|s| s.is_added)
            .map(|s| (s.name.as_str(), s.file.as_path()))
            .collect();

        for file in &changes.files {
            for (sym_name, sym_file) in &symbol_files {
                // Skip self-references and short names that cause false positives
                if file.path.as_path() == *sym_file || sym_name.len() < 4 {
                    continue;
                }

                let call_pattern = format!("{}(", sym_name);
                let has_call = file.diff.lines().any(|line| {
                    line.starts_with('+')
                        && !line.starts_with("+++")
                        && line.contains(&call_pattern)
                });

                if has_call {
                    let caller = file
                        .path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?");
                    connections.push(format!("{} calls {}() — both changed", caller, sym_name));
                }

                if connections.len() >= 5 {
                    break;
                }
            }
            if connections.len() >= 5 {
                break;
            }
        }

        connections.sort();
        connections.dedup();
        connections.truncate(5);
        connections
    }

    /// Detect added/removed import statements from diff lines.
    fn detect_import_changes(changes: &StagedChanges) -> Vec<String> {
        let mut imports = Vec::new();

        for file in &changes.files {
            for line in file.diff.lines() {
                // Skip diff headers
                if line.starts_with("+++") || line.starts_with("---") {
                    continue;
                }
                // Detect added/removed import lines
                if (line.starts_with('+') && Self::is_import_line(&line[1..]))
                    || (line.starts_with('-') && Self::is_import_line(&line[1..]))
                {
                    let action = if line.starts_with('+') {
                        "added"
                    } else {
                        "removed"
                    };
                    let stem = file
                        .path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?");
                    let content = line[1..].trim();
                    imports.push(format!("{}: {} {}", stem, action, content));
                }
            }
        }

        imports.truncate(10); // Cap to avoid prompt bloat
        imports
    }

    fn is_import_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with("use ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("from ")
            || trimmed.starts_with("require(")
            || trimmed.starts_with("#include") // C/C++
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
