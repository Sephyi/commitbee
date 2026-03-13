// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use tokio::process::Command;
use tracing::debug;

/// Minimum number of commits required for meaningful style learning.
const MIN_COMMITS_FOR_ANALYSIS: usize = 5;

/// Maximum number of sample subjects to include in the prompt.
const MAX_SAMPLE_SUBJECTS: usize = 5;

/// Extracted style patterns from the repository's commit history.
#[derive(Debug, Clone)]
pub struct HistoryContext {
    /// Most common commit types with counts, sorted by frequency descending.
    pub type_distribution: Vec<(String, usize)>,
    /// Most common scopes with counts, sorted by frequency descending.
    pub scope_patterns: Vec<(String, usize)>,
    /// Average subject length (characters).
    pub avg_subject_length: usize,
    /// Whether the project consistently uses lowercase subjects.
    pub uses_lowercase: bool,
    /// What percentage of commits follow conventional commit format.
    pub conventional_ratio: f32,
    /// Sample of recent conventional commit subjects for style reference.
    pub sample_subjects: Vec<String>,
}

impl HistoryContext {
    /// Format the history context as a prompt section for the LLM.
    #[must_use]
    pub fn to_prompt_section(&self, sample_size: usize) -> String {
        let mut output = format!("PROJECT STYLE (from last {} commits):", sample_size);

        // Type distribution
        if !self.type_distribution.is_empty() {
            let total: usize = self.type_distribution.iter().map(|(_, c)| c).sum();
            let type_parts: Vec<String> = self
                .type_distribution
                .iter()
                .map(|(t, c)| {
                    let pct = if total > 0 {
                        (*c as f32 / total as f32 * 100.0) as u32
                    } else {
                        0
                    };
                    format!("{} ({}%)", t, pct)
                })
                .collect();
            output.push_str(&format!("\n- Type usage: {}", type_parts.join(", ")));
        }

        // Scope patterns
        if !self.scope_patterns.is_empty() {
            let scope_names: Vec<&str> = self
                .scope_patterns
                .iter()
                .map(|(s, _)| s.as_str())
                .collect();
            output.push_str(&format!("\n- Common scopes: {}", scope_names.join(", ")));
        }

        // Subject style
        let case_style = if self.uses_lowercase {
            "lowercase"
        } else {
            "mixed case"
        };
        output.push_str(&format!(
            "\n- Subject style: {}, avg {} chars",
            case_style, self.avg_subject_length
        ));

        // Conventional compliance
        output.push_str(&format!(
            "\n- Conventional compliance: {:.0}%",
            self.conventional_ratio * 100.0
        ));

        // Sample subjects
        if !self.sample_subjects.is_empty() {
            output.push_str("\n- Recent examples:");
            for subject in &self.sample_subjects {
                output.push_str(&format!("\n  - {}", subject));
            }
        }

        output
    }
}

impl fmt::Display for HistoryContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_prompt_section(50))
    }
}

pub struct HistoryService;

impl HistoryService {
    /// Analyze recent commit history to extract style patterns.
    ///
    /// Returns `None` if the repository has fewer than 5 commits or if
    /// git log fails (e.g., empty repository).
    pub async fn analyze(work_dir: &Path, sample_size: usize) -> Option<HistoryContext> {
        let subjects = Self::fetch_subjects(work_dir, sample_size).await?;

        if subjects.len() < MIN_COMMITS_FOR_ANALYSIS {
            debug!(
                count = subjects.len(),
                min = MIN_COMMITS_FOR_ANALYSIS,
                "too few commits for history learning"
            );
            return None;
        }

        Some(Self::analyze_subjects(&subjects))
    }

    /// Fetch recent commit subject lines via `git log`.
    async fn fetch_subjects(work_dir: &Path, sample_size: usize) -> Option<Vec<String>> {
        let n_arg = sample_size.to_string();
        let output: std::process::Output = Command::new("git")
            .args(["log", "--format=%s", "-n", &n_arg])
            .current_dir(work_dir)
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            debug!("git log failed — possibly empty repository");
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let subjects: Vec<String> = stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();

        Some(subjects)
    }

    /// Parse and analyze a list of commit subjects to extract patterns.
    ///
    /// Public for testing; this is a pure function that does not require git.
    pub fn analyze_subjects(subjects: &[String]) -> HistoryContext {
        let mut type_counts: HashMap<String, usize> = HashMap::new();
        let mut scope_counts: HashMap<String, usize> = HashMap::new();
        let mut total_subject_len: usize = 0;
        let mut lowercase_count: usize = 0;
        let mut conventional_count: usize = 0;
        let mut sample_subjects: Vec<String> = Vec::new();

        for subject in subjects {
            total_subject_len += subject.len();

            if let Some(parsed) = Self::parse_conventional(subject) {
                conventional_count += 1;

                *type_counts.entry(parsed.commit_type).or_default() += 1;

                if let Some(scope) = parsed.scope {
                    *scope_counts.entry(scope).or_default() += 1;
                }

                // Check if the subject text starts lowercase
                if parsed
                    .subject_text
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_lowercase())
                {
                    lowercase_count += 1;
                }

                // Collect sample conventional subjects
                if sample_subjects.len() < MAX_SAMPLE_SUBJECTS {
                    sample_subjects.push(subject.clone());
                }
            } else {
                // Non-conventional: still check case of first char
                if subject.chars().next().is_some_and(|c| c.is_lowercase()) {
                    lowercase_count += 1;
                }
            }
        }

        let total = subjects.len();
        let avg_subject_length = if total > 0 {
            total_subject_len / total
        } else {
            0
        };

        // Lowercase threshold: 80%+ of commits use lowercase
        let uses_lowercase = total > 0 && (lowercase_count as f32 / total as f32) >= 0.8;

        let conventional_ratio = if total > 0 {
            conventional_count as f32 / total as f32
        } else {
            0.0
        };

        // Sort type distribution by count descending
        let mut type_distribution: Vec<(String, usize)> = type_counts.into_iter().collect();
        type_distribution.sort_by(|a, b| b.1.cmp(&a.1));

        // Sort scope patterns by count descending, take top 10
        let mut scope_patterns: Vec<(String, usize)> = scope_counts.into_iter().collect();
        scope_patterns.sort_by(|a, b| b.1.cmp(&a.1));
        scope_patterns.truncate(10);

        HistoryContext {
            type_distribution,
            scope_patterns,
            avg_subject_length,
            uses_lowercase,
            conventional_ratio,
            sample_subjects,
        }
    }

    /// Parse a conventional commit subject line into its components.
    ///
    /// Format: `type(scope): subject` or `type: subject` or `type!: subject`
    fn parse_conventional(subject: &str) -> Option<ParsedConventional> {
        // Find the colon separator
        let colon_pos = subject.find(':')?;
        let prefix = &subject[..colon_pos];

        // Strip trailing `!` (breaking change indicator)
        let prefix = prefix.strip_suffix('!').unwrap_or(prefix);

        // Extract type and optional scope
        let (commit_type, scope) = if let Some(paren_start) = prefix.find('(') {
            let paren_end = prefix.find(')')?;
            if paren_end <= paren_start + 1 {
                return None; // Empty scope
            }
            let t = &prefix[..paren_start];
            let s = &prefix[paren_start + 1..paren_end];
            (t, Some(s))
        } else {
            (prefix, None)
        };

        // Validate the type is a known conventional commit type
        let type_lower = commit_type.to_lowercase();
        if !is_conventional_type(&type_lower) {
            return None;
        }

        // Extract subject text (after ": ")
        let subject_text = subject[colon_pos + 1..].trim();

        Some(ParsedConventional {
            commit_type: type_lower,
            scope: scope.map(|s| s.to_string()),
            subject_text: subject_text.to_string(),
        })
    }
}

struct ParsedConventional {
    commit_type: String,
    scope: Option<String>,
    subject_text: String,
}

fn is_conventional_type(s: &str) -> bool {
    matches!(
        s,
        "feat"
            | "fix"
            | "refactor"
            | "chore"
            | "docs"
            | "test"
            | "style"
            | "perf"
            | "build"
            | "ci"
            | "revert"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_conventional_basic() {
        let parsed = HistoryService::parse_conventional("feat: add new feature").unwrap();
        assert_eq!(parsed.commit_type, "feat");
        assert!(parsed.scope.is_none());
        assert_eq!(parsed.subject_text, "add new feature");
    }

    #[test]
    fn parse_conventional_with_scope() {
        let parsed = HistoryService::parse_conventional("fix(auth): handle null token").unwrap();
        assert_eq!(parsed.commit_type, "fix");
        assert_eq!(parsed.scope.as_deref(), Some("auth"));
        assert_eq!(parsed.subject_text, "handle null token");
    }

    #[test]
    fn parse_conventional_with_breaking() {
        let parsed =
            HistoryService::parse_conventional("refactor(api)!: remove deprecated endpoint")
                .unwrap();
        assert_eq!(parsed.commit_type, "refactor");
        assert_eq!(parsed.scope.as_deref(), Some("api"));
        assert_eq!(parsed.subject_text, "remove deprecated endpoint");
    }

    #[test]
    fn parse_conventional_not_conventional() {
        assert!(HistoryService::parse_conventional("Update README").is_none());
        assert!(HistoryService::parse_conventional("Merge branch 'main'").is_none());
        assert!(HistoryService::parse_conventional("").is_none());
    }

    #[test]
    fn parse_conventional_unknown_type() {
        assert!(HistoryService::parse_conventional("yolo: something").is_none());
    }

    #[test]
    fn analyze_subjects_basic() {
        let subjects = vec![
            "feat(auth): add OAuth2 token refresh".to_string(),
            "fix(api): handle null response from payment gateway".to_string(),
            "refactor(db): migrate from raw SQL to query builder".to_string(),
            "feat: add user profile page".to_string(),
            "chore: update dependencies".to_string(),
            "docs: update README".to_string(),
            "fix(auth): fix session expiry handling".to_string(),
        ];

        let ctx = HistoryService::analyze_subjects(&subjects);

        // Type distribution
        assert!(!ctx.type_distribution.is_empty());
        // feat should be first (appears 2 times)
        assert_eq!(ctx.type_distribution[0].0, "feat");
        assert_eq!(ctx.type_distribution[0].1, 2);

        // Scope patterns
        assert!(!ctx.scope_patterns.is_empty());
        // auth appears 2 times
        assert_eq!(ctx.scope_patterns[0].0, "auth");
        assert_eq!(ctx.scope_patterns[0].1, 2);

        // All subjects start lowercase
        assert!(ctx.uses_lowercase);

        // All are conventional
        assert!((ctx.conventional_ratio - 1.0).abs() < f32::EPSILON);

        // Sample subjects
        assert_eq!(ctx.sample_subjects.len(), 5); // MAX_SAMPLE_SUBJECTS
    }

    #[test]
    fn analyze_subjects_non_conventional() {
        let subjects = vec![
            "Update README with new instructions".to_string(),
            "Fix typo in configuration".to_string(),
            "Add new endpoint for users".to_string(),
            "Remove old migration scripts".to_string(),
            "Merge branch 'feature/auth'".to_string(),
        ];

        let ctx = HistoryService::analyze_subjects(&subjects);

        // No conventional types detected
        assert!(ctx.type_distribution.is_empty());
        assert!(ctx.scope_patterns.is_empty());
        assert!(ctx.conventional_ratio < f32::EPSILON);

        // Average length is still computed
        assert!(ctx.avg_subject_length > 0);

        // Mixed case (all start uppercase)
        assert!(!ctx.uses_lowercase);

        // No sample subjects (none are conventional)
        assert!(ctx.sample_subjects.is_empty());
    }

    #[test]
    fn analyze_subjects_empty() {
        let ctx = HistoryService::analyze_subjects(&[]);
        assert!(ctx.type_distribution.is_empty());
        assert!(ctx.scope_patterns.is_empty());
        assert_eq!(ctx.avg_subject_length, 0);
        assert!(!ctx.uses_lowercase);
        assert!(ctx.conventional_ratio < f32::EPSILON);
        assert!(ctx.sample_subjects.is_empty());
    }

    #[test]
    fn analyze_subjects_mixed_conventional_and_not() {
        let subjects = vec![
            "feat: add search".to_string(),
            "Update docs".to_string(),
            "fix(api): null check".to_string(),
            "Bump version".to_string(),
            "refactor: cleanup imports".to_string(),
        ];

        let ctx = HistoryService::analyze_subjects(&subjects);

        // 3 out of 5 are conventional
        assert!((ctx.conventional_ratio - 0.6).abs() < 0.01);

        // 3 lowercase (conventional) + 0 non-conventional lowercase = 3/5 = 60%
        assert!(!ctx.uses_lowercase); // Below 80% threshold
    }

    #[test]
    fn to_prompt_section_format() {
        let ctx = HistoryContext {
            type_distribution: vec![
                ("feat".to_string(), 4),
                ("fix".to_string(), 3),
                ("refactor".to_string(), 2),
                ("chore".to_string(), 1),
            ],
            scope_patterns: vec![
                ("auth".to_string(), 3),
                ("api".to_string(), 2),
                ("db".to_string(), 1),
            ],
            avg_subject_length: 45,
            uses_lowercase: true,
            conventional_ratio: 0.85,
            sample_subjects: vec![
                "feat(auth): add OAuth2 token refresh".to_string(),
                "fix(api): handle null response".to_string(),
            ],
        };

        let section = ctx.to_prompt_section(50);

        assert!(section.contains("PROJECT STYLE (from last 50 commits):"));
        assert!(section.contains("Type usage:"));
        assert!(section.contains("feat (40%)"));
        assert!(section.contains("Common scopes: auth, api, db"));
        assert!(section.contains("lowercase"));
        assert!(section.contains("avg 45 chars"));
        assert!(section.contains("85%"));
        assert!(section.contains("Recent examples:"));
        assert!(section.contains("feat(auth): add OAuth2 token refresh"));
    }
}
