// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use commitbee::services::history::{HistoryContext, HistoryService};
use std::path::Path;
use std::process::Command;

// ─── Subject Analysis (Pure Functions) ───────────────────────────────────────

#[test]
fn type_distribution_counts_correctly() {
    let subjects = vec![
        "feat: add feature A".to_string(),
        "feat: add feature B".to_string(),
        "feat: add feature C".to_string(),
        "fix: resolve crash".to_string(),
        "fix: handle edge case".to_string(),
        "refactor: cleanup code".to_string(),
        "docs: update guide".to_string(),
        "chore: update deps".to_string(),
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);

    assert_eq!(ctx.type_distribution[0], ("feat".to_string(), 3));
    assert_eq!(ctx.type_distribution[1], ("fix".to_string(), 2));
    // refactor, docs, chore each appear once
    assert_eq!(ctx.type_distribution.len(), 5);
}

#[test]
fn scope_extraction_from_conventional_commits() {
    let subjects = vec![
        "feat(auth): add login".to_string(),
        "fix(auth): fix token".to_string(),
        "feat(api): add endpoint".to_string(),
        "fix(api): null check".to_string(),
        "fix(api): timeout".to_string(),
        "refactor(db): cleanup".to_string(),
        "chore: update deps".to_string(), // no scope
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);

    // api appears 3 times, auth appears 2 times, db appears 1 time
    assert_eq!(ctx.scope_patterns[0], ("api".to_string(), 3));
    assert_eq!(ctx.scope_patterns[1], ("auth".to_string(), 2));
    assert_eq!(ctx.scope_patterns[2], ("db".to_string(), 1));
}

#[test]
fn lowercase_detection_all_lowercase() {
    let subjects = vec![
        "feat: add search".to_string(),
        "fix: resolve issue".to_string(),
        "refactor: cleanup".to_string(),
        "docs: update readme".to_string(),
        "chore: update deps".to_string(),
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);
    assert!(
        ctx.uses_lowercase,
        "all lowercase subjects should be detected"
    );
}

#[test]
fn lowercase_detection_mixed_case() {
    let subjects = vec![
        "feat: Add search".to_string(),
        "fix: Resolve issue".to_string(),
        "refactor: cleanup".to_string(),
        "docs: update readme".to_string(),
        "chore: update deps".to_string(),
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);
    // 3 out of 5 start lowercase (60%), below 80% threshold
    assert!(
        !ctx.uses_lowercase,
        "mixed case (60% lowercase) should not flag as lowercase"
    );
}

#[test]
fn conventional_ratio_all_conventional() {
    let subjects = vec![
        "feat: one".to_string(),
        "fix: two".to_string(),
        "refactor: three".to_string(),
        "docs: four".to_string(),
        "test: five".to_string(),
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);
    assert!(
        (ctx.conventional_ratio - 1.0).abs() < f32::EPSILON,
        "all conventional commits should have ratio 1.0"
    );
}

#[test]
fn conventional_ratio_none_conventional() {
    let subjects = vec![
        "Update README".to_string(),
        "Fix typo".to_string(),
        "Add feature".to_string(),
        "Remove old code".to_string(),
        "Merge branch".to_string(),
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);
    assert!(
        ctx.conventional_ratio < f32::EPSILON,
        "no conventional commits should have ratio 0.0"
    );
}

#[test]
fn conventional_ratio_partial() {
    let subjects = vec![
        "feat: add search".to_string(),
        "Update README".to_string(),
        "fix: crash".to_string(),
        "Merge branch".to_string(),
        "refactor: cleanup".to_string(),
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);
    // 3 out of 5 = 0.6
    assert!(
        (ctx.conventional_ratio - 0.6).abs() < 0.01,
        "3/5 conventional should have ratio 0.6, got {}",
        ctx.conventional_ratio
    );
}

#[test]
fn average_subject_length() {
    let subjects = vec![
        "feat: a".to_string(),     // 7 chars
        "fix: bb".to_string(),     // 7 chars
        "docs: ccc".to_string(),   // 9 chars
        "chore: dddd".to_string(), // 11 chars
        "test: e".to_string(),     // 7 chars
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);
    // Total = 7+7+9+11+7 = 41, avg = 41/5 = 8
    assert_eq!(ctx.avg_subject_length, 8);
}

#[test]
fn empty_subjects_returns_zero_defaults() {
    let ctx = HistoryService::analyze_subjects(&[]);

    assert!(ctx.type_distribution.is_empty());
    assert!(ctx.scope_patterns.is_empty());
    assert_eq!(ctx.avg_subject_length, 0);
    assert!(!ctx.uses_lowercase);
    assert!(ctx.conventional_ratio < f32::EPSILON);
    assert!(ctx.sample_subjects.is_empty());
}

#[test]
fn non_conventional_subjects_still_extract_style() {
    let subjects = vec![
        "Update README with setup instructions".to_string(),
        "Fix database connection timeout".to_string(),
        "Add user profile endpoint".to_string(),
        "Remove deprecated API calls".to_string(),
        "Improve error handling in auth".to_string(),
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);

    // No conventional types, but still has style info
    assert!(ctx.type_distribution.is_empty());
    assert!(ctx.scope_patterns.is_empty());
    assert!(ctx.avg_subject_length > 0);
    // All start uppercase
    assert!(!ctx.uses_lowercase);
}

#[test]
fn sample_subjects_capped_at_five() {
    let subjects: Vec<String> = (0..20)
        .map(|i| format!("feat: feature number {}", i))
        .collect();

    let ctx = HistoryService::analyze_subjects(&subjects);
    assert_eq!(
        ctx.sample_subjects.len(),
        5,
        "sample subjects should be capped at 5"
    );
}

#[test]
fn breaking_change_indicator_parsed() {
    let subjects = vec![
        "feat!: breaking feature".to_string(),
        "refactor(api)!: remove endpoint".to_string(),
        "fix: normal fix".to_string(),
        "feat: normal feat".to_string(),
        "chore: cleanup".to_string(),
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);

    // All 5 are conventional (breaking indicator should be stripped for parsing)
    assert!(
        (ctx.conventional_ratio - 1.0).abs() < f32::EPSILON,
        "commits with ! should still be parsed as conventional"
    );
}

#[test]
fn scope_patterns_sorted_by_frequency() {
    let subjects = vec![
        "feat(z): one".to_string(),
        "feat(a): two".to_string(),
        "feat(a): three".to_string(),
        "feat(a): four".to_string(),
        "feat(m): five".to_string(),
        "feat(m): six".to_string(),
    ];

    let ctx = HistoryService::analyze_subjects(&subjects);

    // a=3, m=2, z=1
    assert_eq!(ctx.scope_patterns[0].0, "a");
    assert_eq!(ctx.scope_patterns[1].0, "m");
    assert_eq!(ctx.scope_patterns[2].0, "z");
}

// ─── Prompt Section Formatting ───────────────────────────────────────────────

#[test]
fn prompt_section_includes_all_components() {
    let ctx = HistoryContext {
        type_distribution: vec![("feat".to_string(), 5), ("fix".to_string(), 3)],
        scope_patterns: vec![("auth".to_string(), 3)],
        avg_subject_length: 40,
        uses_lowercase: true,
        conventional_ratio: 0.9,
        sample_subjects: vec!["feat(auth): add login flow".to_string()],
    };

    let section = ctx.to_prompt_section(50);

    assert!(section.contains("PROJECT STYLE"));
    assert!(section.contains("Type usage:"));
    assert!(section.contains("Common scopes:"));
    assert!(section.contains("Subject style:"));
    assert!(section.contains("Conventional compliance:"));
    assert!(section.contains("Recent examples:"));
}

#[test]
fn prompt_section_no_scopes_omits_scope_line() {
    let ctx = HistoryContext {
        type_distribution: vec![("feat".to_string(), 5)],
        scope_patterns: vec![],
        avg_subject_length: 30,
        uses_lowercase: true,
        conventional_ratio: 1.0,
        sample_subjects: vec![],
    };

    let section = ctx.to_prompt_section(50);

    assert!(!section.contains("Common scopes:"));
}

#[test]
fn prompt_section_no_samples_omits_examples() {
    let ctx = HistoryContext {
        type_distribution: vec![("feat".to_string(), 5)],
        scope_patterns: vec![],
        avg_subject_length: 30,
        uses_lowercase: false,
        conventional_ratio: 1.0,
        sample_subjects: vec![],
    };

    let section = ctx.to_prompt_section(25);

    assert!(section.contains("from last 25 commits"));
    assert!(!section.contains("Recent examples:"));
}

#[test]
fn prompt_section_percentage_calculation() {
    let ctx = HistoryContext {
        type_distribution: vec![
            ("feat".to_string(), 10),
            ("fix".to_string(), 5),
            ("refactor".to_string(), 5),
        ],
        scope_patterns: vec![],
        avg_subject_length: 40,
        uses_lowercase: true,
        conventional_ratio: 0.8,
        sample_subjects: vec![],
    };

    let section = ctx.to_prompt_section(50);

    // feat = 10/20 = 50%, fix = 5/20 = 25%, refactor = 5/20 = 25%
    assert!(section.contains("feat (50%)"));
    assert!(section.contains("fix (25%)"));
    assert!(section.contains("refactor (25%)"));
}

// ─── Git Integration (requires tempdir with git repo) ────────────────────────

/// Builds a `git` invocation that is hermetic against ambient git
/// configuration. Disables system/global config, redirects `HOME` to
/// the test's own tempdir, and pre-supplies author/committer identity
/// so commits succeed even when the host has no `user.email` set or
/// has `commit.gpgsign=true` globally.
fn hermetic_git(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("HOME", dir)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com");
    cmd
}

#[tokio::test]
async fn analyze_repo_with_enough_commits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    hermetic_git(path).args(["init"]).output().unwrap();

    // Create 6 commits (above MIN_COMMITS_FOR_ANALYSIS = 5)
    let commit_subjects = [
        "feat(auth): add login endpoint",
        "fix(auth): handle expired tokens",
        "refactor(db): use connection pool",
        "feat(api): add user search",
        "docs: update API guide",
        "chore: update dependencies",
    ];

    for (i, subject) in commit_subjects.iter().enumerate() {
        let file = path.join(format!("file_{}.txt", i));
        std::fs::write(&file, format!("content {}", i)).unwrap();

        hermetic_git(path).args(["add", "."]).output().unwrap();

        hermetic_git(path)
            .args(["commit", "-m", subject])
            .output()
            .unwrap();
    }

    let result = HistoryService::analyze(path, 50).await;
    assert!(result.is_some(), "should succeed with 6 commits");

    let ctx = result.unwrap();
    assert!(!ctx.type_distribution.is_empty());
    assert!(!ctx.scope_patterns.is_empty());
    assert!(ctx.uses_lowercase);
    assert!((ctx.conventional_ratio - 1.0).abs() < f32::EPSILON);
}

#[tokio::test]
async fn analyze_repo_with_too_few_commits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    hermetic_git(path).args(["init"]).output().unwrap();

    // Create only 3 commits (below MIN_COMMITS_FOR_ANALYSIS = 5)
    for i in 0..3 {
        let file = path.join(format!("file_{}.txt", i));
        std::fs::write(&file, format!("content {}", i)).unwrap();

        hermetic_git(path).args(["add", "."]).output().unwrap();

        hermetic_git(path)
            .args(["commit", "-m", &format!("feat: feature {}", i)])
            .output()
            .unwrap();
    }

    let result = HistoryService::analyze(path, 50).await;
    assert!(result.is_none(), "should return None with < 5 commits");
}

#[tokio::test]
async fn analyze_empty_repo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    hermetic_git(path).args(["init"]).output().unwrap();

    let result = HistoryService::analyze(path, 50).await;
    assert!(result.is_none(), "should return None for empty repo");
}

#[tokio::test]
async fn analyze_non_git_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    // Don't init git
    let result = HistoryService::analyze(path, 50).await;
    assert!(result.is_none(), "should return None for non-git directory");
}

#[tokio::test]
async fn analyze_respects_sample_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    hermetic_git(path).args(["init"]).output().unwrap();

    // Create 10 commits
    for i in 0..10 {
        let file = path.join(format!("file_{}.txt", i));
        std::fs::write(&file, format!("content {}", i)).unwrap();

        hermetic_git(path).args(["add", "."]).output().unwrap();

        hermetic_git(path)
            .args(["commit", "-m", &format!("feat: feature {}", i)])
            .output()
            .unwrap();
    }

    // Request only 5 commits
    let result = HistoryService::analyze(path, 5).await;
    assert!(result.is_some());

    let ctx = result.unwrap();
    // The type distribution total should be exactly 5
    let total: usize = ctx.type_distribution.iter().map(|(_, c)| c).sum();
    assert_eq!(total, 5, "should only analyze 5 commits");
}
