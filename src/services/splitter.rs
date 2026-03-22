// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::collections::{HashMap, HashSet};
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
const GENERIC_DIRS: &[&str] = &[
    "src",
    "lib",
    "services",
    "domain",
    "utils",
    "helpers",
    "internal",
    "core",
    "pkg",
    "cmd",
    "",
    "app",
    "api",
    "modules",
    "components",
    "common",
    "shared",
    "middleware",
    "handlers",
    "controllers",
    "models",
    "views",
    "routes",
];

pub struct CommitSplitter;

impl CommitSplitter {
    /// Analyze staged changes and determine if they should be split.
    ///
    /// Strategy:
    /// 1. Separate files by category (source, test, docs, config/build)
    /// 2. Group source files by diff-shape similarity (cross-file pattern detection)
    /// 3. Merge groups connected by symbol dependencies
    /// 4. Attach test files to matching source groups
    /// 5. Keep docs and config/build as their own groups when mixed with source
    pub fn analyze(changes: &StagedChanges, symbols: &[CodeSymbol]) -> SplitSuggestion {
        // Classify files by category
        let mut source_files: Vec<&FileChange> = Vec::new();
        let mut test_files: Vec<&FileChange> = Vec::new();
        let mut doc_files: Vec<&FileChange> = Vec::new();
        let mut config_build_files: Vec<&FileChange> = Vec::new();

        for file in &changes.files {
            match file.category {
                FileCategory::Source => source_files.push(file),
                FileCategory::Test => test_files.push(file),
                FileCategory::Docs => doc_files.push(file),
                FileCategory::Config | FileCategory::Build => config_build_files.push(file),
                FileCategory::Other => config_build_files.push(file),
            }
        }

        // If no source files, no meaningful split possible
        if source_files.is_empty() {
            return SplitSuggestion::SingleCommit;
        }

        // Step 1: Group source files by diff-shape similarity
        let mut source_groups = Self::group_by_diff_shape(&source_files);

        // Step 2: Within each shape group, sub-group by module if shapes differ
        // (files in the same shape group stay together even if in different modules)

        // Step 3: Merge groups connected by symbol dependencies
        Self::merge_by_symbol_deps(&mut source_groups, symbols);

        // Step 4: Attach test files to matching source groups
        Self::attach_test_files(&mut source_groups, &test_files);

        // Step 5: Build final groups with scored support file assignment
        let mut all_groups: Vec<Vec<&FileChange>> = source_groups;

        // Attach support files (docs, config, build) by affinity scoring.
        // Known pairs stick together (Cargo.toml+Cargo.lock), others go to the
        // source group with best keyword overlap, or form a standalone group.
        Self::attach_support_files_scored(&mut all_groups, &doc_files);
        Self::attach_support_files_scored(&mut all_groups, &config_build_files);

        // If only 1 group total, no split needed
        if all_groups.len() <= 1 {
            return SplitSuggestion::SingleCommit;
        }

        // Step 6: Build CommitGroups with type/scope inference
        let mut groups: Vec<CommitGroup> = Vec::new();

        for files in &all_groups {
            let paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
            let sub_changes = changes.subset(&paths);
            let sub_symbols: Vec<CodeSymbol> = symbols
                .iter()
                .filter(|s| paths.contains(&s.file))
                .cloned()
                .collect();

            // Whitespace classification requires full build(); pass false here
            // since sub_symbols are not yet classified via classify_span_change.
            let commit_type = ContextBuilder::infer_commit_type(&sub_changes, &sub_symbols, false);
            let scope = ContextBuilder::infer_scope(&sub_changes);

            groups.push(CommitGroup {
                files: paths,
                commit_type,
                scope,
            });
        }

        // Step 7: Check if groups are actually different
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

    /// Compute a structural fingerprint of a diff for similarity comparison.
    ///
    /// The fingerprint captures the *shape* of changes (what kind of lines were
    /// added/removed, the ratio of additions to deletions) without caring about
    /// specific content. Files with similar fingerprints likely received the same
    /// mechanical transformation.
    fn diff_fingerprint(file: &FileChange) -> DiffFingerprint {
        let lines: Vec<&str> = file.diff.lines().collect();
        // Count structural line categories
        let mut added = 0usize;
        let mut removed = 0usize;
        let mut hunk_headers = 0usize;

        for line in &lines {
            if line.starts_with("@@") {
                hunk_headers += 1;
            } else if line.starts_with('+') && !line.starts_with("+++") {
                added += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                removed += 1;
            }
        }

        // Compute the balance ratio: how symmetric is add vs remove?
        // 0.0 = all adds or all removes, 1.0 = perfectly balanced
        let balance = if added + removed == 0 {
            1.0
        } else {
            let min = added.min(removed) as f64;
            let max = added.max(removed) as f64;
            min / max
        };

        // Detect if changes are purely whitespace/indentation restructuring:
        // lines that differ only in leading whitespace after stripping +/-
        let mut indent_only_changes = 0usize;
        let added_lines: Vec<&str> = lines
            .iter()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
            .map(|l| &l[1..])
            .collect();
        let removed_lines: Vec<&str> = lines
            .iter()
            .filter(|l| l.starts_with('-') && !l.starts_with("---"))
            .map(|l| &l[1..])
            .collect();

        for added_line in &added_lines {
            let trimmed = added_line.trim();
            if removed_lines
                .iter()
                .any(|r| r.trim() == trimmed && *r != *added_line)
            {
                indent_only_changes += 1;
            }
        }

        let indent_ratio = if added > 0 {
            indent_only_changes as f64 / added as f64
        } else {
            0.0
        };

        DiffFingerprint {
            added,
            removed,
            hunk_count: hunk_headers,
            balance,
            indent_ratio,
        }
    }

    /// Compute Jaccard similarity between two sets of tokens.
    fn jaccard_index(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
        if a.is_empty() && b.is_empty() {
            return 1.0;
        }
        let intersection = a.intersection(b).count();
        let union = a.len() + b.len() - intersection;
        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Extract significant tokens from a diff.
    /// Captures the vocabulary of the change (variable names, keywords, types).
    fn tokenize_diff(diff: &str) -> HashSet<String> {
        let mut tokens = HashSet::new();
        for line in diff.lines() {
            // Only process add/remove lines, skip headers
            if (!line.starts_with('+') && !line.starts_with('-'))
                || line.starts_with("+++")
                || line.starts_with("---")
            {
                continue;
            }

            // Split by non-alphanumeric characters to get words
            for word in line[1..].split(|c: char| !c.is_alphanumeric()) {
                if word.len() > 2 {
                    // Skip tiny tokens
                    tokens.insert(word.to_string());
                }
            }
        }
        tokens
    }

    /// Group source files by diff-shape similarity.
    ///
    /// Files with very similar fingerprints (same kind of transformation)
    /// are grouped together even if they're in different modules.
    fn group_by_diff_shape<'a>(files: &[&'a FileChange]) -> Vec<Vec<&'a FileChange>> {
        if files.len() <= 1 {
            return vec![files.to_vec()];
        }

        // Pre-calculate features for all files
        let features: Vec<(&FileChange, DiffFingerprint, HashSet<String>)> = files
            .iter()
            .map(|f| (*f, Self::diff_fingerprint(f), Self::tokenize_diff(&f.diff)))
            .collect();

        // Greedy clustering with hybrid similarity
        let mut clusters: Vec<(DiffFingerprint, HashSet<String>, Vec<&'a FileChange>)> = Vec::new();

        for (file, fp, tokens) in &features {
            let mut assigned = false;

            for (centroid_fp, centroid_tokens, members) in &mut clusters {
                // Hybrid similarity:
                // 1. Must be statistically similar (shape/size)
                // 2. Must share content vocabulary (Jaccard index)

                if centroid_fp.is_similar(fp) {
                    let content_sim = Self::jaccard_index(centroid_tokens, tokens);

                    // Threshold: 0.4 implies significant vocabulary overlap
                    // e.g., sharing variable names, types, or specific syntax patterns
                    if content_sim > 0.4 {
                        members.push(file);
                        // Update centroid tokens? Union them to represent the group better?
                        // For simple greedy, keeping the first file's tokens as centroid is simpler/stable.
                        assigned = true;
                        break;
                    }
                }
            }

            if !assigned {
                clusters.push((fp.clone(), tokens.clone(), vec![file]));
            }
        }

        // If clustering produced only 1 group, check if it's genuinely uniform
        // or if we should fall back to module-based splitting.
        if clusters.len() == 1 {
            let (centroid, _, cluster_files) = &clusters[0];

            // If all files have non-empty, highly balanced diffs (adds ≈ removes) and are
            // small, they likely received the same mechanical transformation → keep grouped.
            let all_balanced_small = cluster_files.len() > 1
                && cluster_files.iter().all(|f| {
                    let fp = Self::diff_fingerprint(f);
                    fp.added + fp.removed > 0 && fp.balance > 0.5 && fp.added + fp.removed < 40
                });

            if !all_balanced_small {
                let modules: Vec<String> =
                    files.iter().map(|f| Self::detect_module(&f.path)).collect();
                let unique_modules = modules
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len();

                if unique_modules > 1 && centroid.indent_ratio < 0.3 {
                    return Self::group_by_module(files);
                }
            }
        }

        let mut result: Vec<Vec<&'a FileChange>> = Vec::new();

        for (_, _, cluster_files) in clusters {
            // E2: Sub-split large clusters that span multiple modules.
            // If a group has >6 files across multiple modules, the shape clustering
            // wasn't discriminating enough — split by module to avoid mega-groups.
            if cluster_files.len() > 6 {
                let modules: std::collections::HashSet<String> = cluster_files
                    .iter()
                    .map(|f| Self::detect_module(&f.path))
                    .collect();

                if modules.len() > 1 {
                    let sub_groups = Self::group_by_module(&cluster_files);
                    result.extend(sub_groups);
                    continue;
                }
            }
            result.push(cluster_files);
        }

        result
    }

    /// Traditional module-based grouping (fallback when diff shapes don't differentiate).
    fn group_by_module<'a>(files: &[&'a FileChange]) -> Vec<Vec<&'a FileChange>> {
        let mut module_files: HashMap<String, Vec<&FileChange>> = HashMap::new();

        for file in files {
            let module = Self::detect_module(&file.path);
            module_files.entry(module).or_default().push(file);
        }

        module_files.into_values().collect()
    }

    /// Merge groups that share symbol dependencies.
    ///
    /// If file A removes a public symbol and file B adds a symbol with the same name,
    /// or if file A's diff references symbols from file B's group, merge them.
    fn merge_by_symbol_deps(groups: &mut Vec<Vec<&FileChange>>, symbols: &[CodeSymbol]) {
        if groups.len() <= 1 {
            return;
        }

        // Build a map: file path → group index
        let mut file_to_group: HashMap<&Path, usize> = HashMap::new();
        for (idx, group) in groups.iter().enumerate() {
            for file in group {
                file_to_group.insert(file.path.as_path(), idx);
            }
        }

        // Find symbol pairs that indicate dependency between groups:
        // A removed symbol in group X and an added symbol with same name in group Y
        let mut merge_pairs: Vec<(usize, usize)> = Vec::new();

        let removed: Vec<_> = symbols.iter().filter(|s| !s.is_added).collect();
        let added: Vec<_> = symbols.iter().filter(|s| s.is_added).collect();

        for rem in &removed {
            for add in &added {
                if rem.name == add.name
                    && rem.kind == add.kind
                    && rem.file != add.file
                    && let (Some(&g1), Some(&g2)) = (
                        file_to_group.get(rem.file.as_path()),
                        file_to_group.get(add.file.as_path()),
                    )
                    && g1 != g2
                {
                    merge_pairs.push((g1.min(g2), g1.max(g2)));
                }
            }
        }

        // Also merge when a file's diff adds a line that directly CALLS a new function
        // from another group. Only matches `+` lines containing `sym_name(` — much more
        // precise than the previous `diff.contains(sym_name)` which caused cascading merges
        // from import statements and type references.
        for (idx, group) in groups.iter().enumerate() {
            for file in group {
                for sym in &added {
                    if let Some(&sym_group) = file_to_group.get(sym.file.as_path())
                        && sym_group != idx
                    {
                        // Only match added lines (`+`) that contain a function call pattern
                        let call_pattern = format!("{}(", sym.name);
                        let has_call = file.diff.lines().any(|line| {
                            line.starts_with('+')
                                && !line.starts_with("+++")
                                && line.contains(&call_pattern)
                        });
                        if has_call {
                            merge_pairs.push((idx.min(sym_group), idx.max(sym_group)));
                        }
                    }
                }
            }
        }

        // Apply merges (deduplicated, in reverse order to preserve indices)
        merge_pairs.sort();
        merge_pairs.dedup();
        merge_pairs.reverse();

        for (keep, merge) in merge_pairs {
            if merge < groups.len() && keep < groups.len() && keep != merge {
                let merged = groups.remove(merge);
                groups[keep].extend(merged);
            }
        }
    }

    /// Detect the "module" for a source file based on its path.
    ///
    /// Uses the most specific directory name, falling back to file stem
    /// when the parent directory is too generic (src, services, lib).
    fn detect_module(path: &Path) -> String {
        // Use parent directory name if it's specific enough
        if let Some(parent) = path.parent()
            && let Some(parent_name) = parent.file_name().and_then(|n| n.to_str())
            && !GENERIC_DIRS.contains(&parent_name)
        {
            return parent_name.to_string();
        }

        // Fall back to file stem (e.g., sanitizer.rs → "sanitizer")
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Attach test files to matching source groups.
    fn attach_test_files<'a>(
        source_groups: &mut Vec<Vec<&'a FileChange>>,
        test_files: &[&'a FileChange],
    ) {
        // Find the largest source group (fallback target)
        let largest_idx = source_groups
            .iter()
            .enumerate()
            .max_by_key(|(_, files)| {
                files
                    .iter()
                    .map(|f| f.additions + f.deletions)
                    .sum::<usize>()
            })
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        for test in test_files {
            let stem = test.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

            // Try to match test file stem to a source file stem in any group
            let target = source_groups
                .iter()
                .position(|group| {
                    group.iter().any(|f| {
                        f.path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .is_some_and(|s| s == stem)
                    })
                })
                .unwrap_or(largest_idx);

            if target < source_groups.len() {
                source_groups[target].push(test);
            }
        }
    }

    /// Known config file pairs that should stick together.
    const KNOWN_PAIRS: &[(&str, &str)] = &[
        ("Cargo.toml", "Cargo.lock"),
        ("package.json", "package-lock.json"),
        ("package.json", "yarn.lock"),
        ("package.json", "pnpm-lock.yaml"),
        ("package.json", "bun.lockb"),
        ("Gemfile", "Gemfile.lock"),
        ("Pipfile", "Pipfile.lock"),
        ("go.mod", "go.sum"),
        ("pubspec.yaml", "pubspec.lock"),
        ("flake.nix", "flake.lock"),
    ];

    /// Attach support files (docs, config, build) to source groups by affinity scoring.
    ///
    /// For each support file, score it against each source group:
    /// - Known pair bonus: if the group already has the pair partner
    /// - Stem/module overlap: file stem appears in group file names or module
    /// - Keyword overlap: diff content shares tokens with group diffs
    ///
    /// If max score < threshold, file goes into a standalone group.
    fn attach_support_files_scored<'a>(
        groups: &mut Vec<Vec<&'a FileChange>>,
        support_files: &[&'a FileChange],
    ) {
        if support_files.is_empty() || groups.is_empty() {
            if !support_files.is_empty() {
                groups.push(support_files.to_vec());
            }
            return;
        }

        let mut standalone: Vec<&'a FileChange> = Vec::new();

        for file in support_files {
            let name = file.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let stem = file.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

            let mut best_score = 0.0_f64;
            let mut best_idx = 0;

            for (idx, group) in groups.iter().enumerate() {
                let mut score = 0.0_f64;

                // Known pair bonus: check if this file's pair partner is in the group
                for (a, b) in Self::KNOWN_PAIRS {
                    let partner = if name == *a {
                        Some(*b)
                    } else if name == *b {
                        Some(*a)
                    } else {
                        None
                    };
                    if let Some(partner_name) = partner
                        && group.iter().any(|f| {
                            f.path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .is_some_and(|n| n == partner_name)
                        })
                    {
                        score += 1.0;
                    }
                }

                // Stem overlap: file stem appears in group file stems or modules
                if !stem.is_empty() {
                    let has_stem_match = group.iter().any(|f| {
                        let f_stem = f.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        f_stem == stem || f.path.to_string_lossy().contains(stem)
                    });
                    if has_stem_match {
                        score += 0.5;
                    }
                }

                if score > best_score {
                    best_score = score;
                    best_idx = idx;
                }
            }

            // Threshold: only attach if affinity is meaningful
            if best_score >= 0.5 {
                groups[best_idx].push(file);
            } else {
                standalone.push(file);
            }
        }

        if !standalone.is_empty() {
            groups.push(standalone);
        }
    }

    /// Calculate total change size for a group (for sorting).
    fn group_change_size(group: &CommitGroup, changes: &StagedChanges) -> usize {
        let group_files: HashSet<&PathBuf> = group.files.iter().collect();
        changes
            .files
            .iter()
            .filter(|f| group_files.contains(&f.path))
            .map(|f| f.additions + f.deletions)
            .sum()
    }
}

/// Structural fingerprint of a diff for similarity comparison.
#[derive(Debug, Clone)]
struct DiffFingerprint {
    added: usize,
    removed: usize,
    hunk_count: usize,
    /// 0.0 = all adds or removes, 1.0 = perfectly balanced
    balance: f64,
    /// Fraction of added lines that differ from a removed line only by indentation
    indent_ratio: f64,
}

impl DiffFingerprint {
    /// Check if two fingerprints represent the same *kind* of change.
    fn is_similar(&self, other: &DiffFingerprint) -> bool {
        // Both must be in the same size class
        let size_a = self.added + self.removed;
        let size_b = other.added + other.removed;

        if size_a == 0 || size_b == 0 {
            return size_a == size_b;
        }

        // Size ratio must be within 3x
        let size_ratio = size_a.max(size_b) as f64 / size_a.min(size_b) as f64;
        if size_ratio > 3.0 {
            return false;
        }

        // Balance must be similar (both balanced or both one-sided)
        if (self.balance - other.balance).abs() > 0.4 {
            return false;
        }

        // If both are high-indent-ratio (>0.5), they're likely the same refactor
        if self.indent_ratio > 0.5 && other.indent_ratio > 0.5 {
            return true;
        }

        // Hunk count should be in similar range
        let hunk_ratio = if self.hunk_count == 0 && other.hunk_count == 0 {
            1.0
        } else if self.hunk_count == 0 || other.hunk_count == 0 {
            0.0
        } else {
            self.hunk_count.min(other.hunk_count) as f64
                / self.hunk_count.max(other.hunk_count) as f64
        };

        hunk_ratio > 0.3
    }
}
