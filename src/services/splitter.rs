// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::domain::{CodeSymbol, CommitType, FileCategory, FileChange, StagedChanges};
use crate::services::context::ContextBuilder;

/// A logical group of files that belong in a single commit.
#[derive(Debug)]
pub struct CommitGroup {
    pub files: Vec<PathBuf>,
    pub commit_type: CommitType,
    pub scope: Option<String>,
}

/// Result of analyzing staged changes for potential splitting.
#[derive(Debug)]
pub enum SplitSuggestion {
    /// All files belong in a single commit.
    SingleCommit,
    /// Files should be split into multiple commits.
    SuggestSplit(Vec<CommitGroup>),
}

/// Directories considered "generic" — too broad to be a meaningful module name.
const GENERIC_DIRS: &[&str] = &["src", "lib", "services", ""];

pub struct CommitSplitter;

impl CommitSplitter {
    /// Analyze staged changes and determine if they should be split.
    pub fn analyze(changes: &StagedChanges, symbols: &[CodeSymbol]) -> SplitSuggestion {
        // Step 1: Group source files by module
        let mut module_files: HashMap<String, Vec<&FileChange>> = HashMap::new();
        let mut support_files: Vec<&FileChange> = Vec::new();

        for file in &changes.files {
            if file.category == FileCategory::Source {
                let module = Self::detect_module(&file.path);
                module_files.entry(module).or_default().push(file);
            } else {
                support_files.push(file);
            }
        }

        // If 0 or 1 source modules, no split needed
        if module_files.len() <= 1 {
            return SplitSuggestion::SingleCommit;
        }

        // Step 2: Attach support files to source groups
        Self::attach_support_files(&mut module_files, &support_files);

        // Step 3: Build CommitGroups with type/scope inference
        let mut groups: Vec<CommitGroup> = Vec::new();

        for files in module_files.values() {
            let paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
            let sub_changes = changes.subset(&paths);
            let sub_symbols: Vec<CodeSymbol> = symbols
                .iter()
                .filter(|s| paths.contains(&s.file))
                .cloned()
                .collect();

            let commit_type = ContextBuilder::infer_commit_type(&sub_changes, &sub_symbols);
            let scope = ContextBuilder::infer_scope(&sub_changes);

            groups.push(CommitGroup {
                files: paths,
                commit_type,
                scope,
            });
        }

        // Step 4: Check if groups are actually different
        // If all groups have the same type AND scope, no split needed
        if groups.len() >= 2 {
            let first_type = groups[0].commit_type;
            let first_scope = &groups[0].scope;
            let all_same = groups
                .iter()
                .all(|g| g.commit_type == first_type && &g.scope == first_scope);

            if all_same {
                return SplitSuggestion::SingleCommit;
            }
        }

        // Sort groups by total change size (largest first)
        groups.sort_by(|a, b| {
            let a_size = Self::group_change_size(a, changes);
            let b_size = Self::group_change_size(b, changes);
            b_size.cmp(&a_size)
        });

        SplitSuggestion::SuggestSplit(groups)
    }

    /// Detect the "module" for a source file based on its path.
    ///
    /// Uses the most specific directory name, falling back to file stem
    /// when the parent directory is too generic (src, services, lib).
    fn detect_module(path: &Path) -> String {
        // Use parent directory name if it's specific enough
        if let Some(parent) = path.parent() {
            if let Some(parent_name) = parent.file_name().and_then(|n| n.to_str()) {
                if !GENERIC_DIRS.contains(&parent_name) {
                    return parent_name.to_string();
                }
            }
        }

        // Fall back to file stem (e.g., sanitizer.rs → "sanitizer")
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Attach non-source files (tests, docs, config, etc.) to source groups.
    fn attach_support_files<'a>(
        module_files: &mut HashMap<String, Vec<&'a FileChange>>,
        support_files: &[&'a FileChange],
    ) {
        // Find the largest source group by total additions+deletions
        let largest_module = module_files
            .iter()
            .max_by_key(|(_, files)| {
                files
                    .iter()
                    .map(|f| f.additions + f.deletions)
                    .sum::<usize>()
            })
            .map(|(name, _)| name.clone());

        let Some(largest) = largest_module else {
            return;
        };

        for file in support_files {
            let target = match file.category {
                FileCategory::Test => {
                    // Try to match test file stem to a source module name
                    let stem = file.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

                    if module_files.contains_key(stem) {
                        stem.to_string()
                    } else {
                        largest.clone()
                    }
                }
                // Config, docs, build, other → attach to largest group
                _ => largest.clone(),
            };

            module_files.entry(target).or_default().push(file);
        }
    }

    /// Calculate total change size for a group (for sorting).
    fn group_change_size(group: &CommitGroup, changes: &StagedChanges) -> usize {
        changes
            .files
            .iter()
            .filter(|f| group.files.contains(&f.path))
            .map(|f| f.additions + f.deletions)
            .sum()
    }
}
