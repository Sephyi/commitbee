// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Evaluation harness for pipeline quality testing.
//!
//! Runs fixture diffs through the context builder, commit type inference,
//! and sanitizer pipeline, then compares against expected output snapshots.
//! Feature-gated behind `eval`.
//!
//! ## Fixture format
//!
//! Each fixture lives in `tests/fixtures/eval/<name>/`:
//! - `metadata.toml` — required: assertions and expectations
//! - `diff.patch` — required: unified diff
//! - `symbols.toml` — optional: injected `CodeSymbol` data for AST testing
//! - `response.json` — optional: canned LLM response for sanitizer testing
//! - `config.toml` — optional: config overrides
//! - `expected.txt` — optional: expected first line of sanitized message

use std::path::{Path, PathBuf};
use std::sync::Arc;

use console::style;
use serde::Deserialize;

use crate::config::Config;
use crate::domain::{
    ChangeStatus, CodeSymbol, DiffStats, FileCategory, FileChange, StagedChanges, SymbolKind,
};
use crate::error::{Error, Result};
use crate::services::context::ContextBuilder;
use crate::services::sanitizer::CommitSanitizer;

/// Metadata describing a single evaluation fixture.
#[derive(Debug, Deserialize)]
pub struct FixtureMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub language: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub category: Option<String>,
    pub expected_type: String,
    #[serde(default)]
    pub expected_scope: Option<String>,

    /// Evidence flag assertions (all optional).
    #[serde(default)]
    pub evidence: Option<EvidenceExpectation>,

    /// Prompt content assertions.
    #[serde(default)]
    pub prompt: Option<PromptExpectation>,

    /// Cross-file connection assertions.
    #[serde(default)]
    pub connections: Option<ConnectionsExpectation>,

    /// Subject line assertions.
    #[serde(default)]
    pub subject: Option<SubjectExpectation>,

    /// Breaking change assertions.
    #[serde(default)]
    pub breaking: Option<BreakingExpectation>,
}

/// Expected evidence flags from the context builder.
#[derive(Debug, Default, Deserialize)]
pub struct EvidenceExpectation {
    #[serde(default)]
    pub is_mechanical: Option<bool>,
    #[serde(default)]
    pub has_bug_evidence: Option<bool>,
    #[serde(default)]
    pub has_new_public_api: Option<bool>,
    #[serde(default)]
    pub public_api_removed_count: Option<usize>,
    #[serde(default)]
    pub is_dependency_only: Option<bool>,
}

/// Expected prompt content patterns.
#[derive(Debug, Default, Deserialize)]
pub struct PromptExpectation {
    #[serde(default)]
    pub must_contain: Vec<String>,
    #[serde(default)]
    pub must_not_contain: Vec<String>,
}

/// Expected cross-file connection assertions.
#[derive(Debug, Default, Deserialize)]
pub struct ConnectionsExpectation {
    #[serde(default)]
    pub expected_count: Option<usize>,
    #[serde(default)]
    pub min_count: Option<usize>,
    #[serde(default)]
    pub must_contain: Vec<String>,
}

/// Expected subject line properties.
#[derive(Debug, Default, Deserialize)]
pub struct SubjectExpectation {
    #[serde(default)]
    pub must_contain: Vec<String>,
    #[serde(default)]
    pub must_not_contain: Vec<String>,
    #[serde(default)]
    pub max_length: Option<usize>,
}

/// Expected breaking change behavior.
#[derive(Debug, Default, Deserialize)]
pub struct BreakingExpectation {
    /// Whether breaking_change metadata signals are expected.
    #[serde(default)]
    pub expected: Option<bool>,
}

/// A single symbol definition from `symbols.toml`.
#[derive(Debug, Deserialize)]
struct SymbolDef {
    kind: String,
    name: String,
    file: String,
    #[serde(default = "default_line")]
    line: usize,
    #[serde(default = "default_line")]
    end_line: usize,
    #[serde(default)]
    is_public: bool,
    #[serde(default)]
    is_added: bool,
    #[serde(default)]
    is_whitespace_only: Option<bool>,
    #[serde(default)]
    signature: Option<String>,
    #[serde(default)]
    parent_scope: Option<String>,
}

fn default_line() -> usize {
    1
}

/// Container for deserializing `symbols.toml`.
#[derive(Debug, Deserialize)]
struct SymbolsFile {
    #[serde(default)]
    symbols: Vec<SymbolDef>,
}

/// Individual assertion failure.
#[derive(Debug, Clone)]
pub struct AssertionFailure {
    pub category: String,
    pub message: String,
}

impl std::fmt::Display for AssertionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.category, self.message)
    }
}

/// Result of running one fixture through the pipeline.
#[derive(Debug)]
pub struct EvalResult {
    pub fixture_name: String,
    pub description: String,
    // Type inference check
    pub expected_type: String,
    pub actual_type: String,
    pub type_passed: bool,
    // Scope check (only if expected_scope is set)
    pub expected_scope: Option<String>,
    pub actual_scope: Option<String>,
    pub scope_passed: bool,
    // Prompt assembly check
    pub prompt_assembled: bool,
    // Sanitizer check (only if response.json exists)
    pub sanitizer_result: Option<SanitizerCheck>,
    // Expected message check (only if expected.txt exists)
    pub message_check: Option<MessageCheck>,
    // New assertion failures (evidence, prompt, connections, subject, breaking)
    pub assertion_failures: Vec<AssertionFailure>,
    // Overall
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct SanitizerCheck {
    pub passed: bool,
    pub actual_message: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct MessageCheck {
    pub expected_first_line: String,
    pub actual_first_line: Option<String>,
    pub passed: bool,
}

impl EvalResult {
    pub fn passed(&self) -> bool {
        self.type_passed
            && self.scope_passed
            && self.prompt_assembled
            && self.error.is_none()
            && self.assertion_failures.is_empty()
            && self.sanitizer_result.as_ref().is_none_or(|s| s.passed)
            && self.message_check.as_ref().is_none_or(|m| m.passed)
    }
}

/// Evaluation runner that processes fixture directories.
pub struct EvalRunner {
    fixtures_dir: PathBuf,
    filter: Option<String>,
}

impl EvalRunner {
    #[must_use]
    pub fn new(fixtures_dir: PathBuf, filter: Option<String>) -> Self {
        Self {
            fixtures_dir,
            filter,
        }
    }

    /// Run all fixtures and return results without calling `process::exit`.
    ///
    /// Suitable for integration tests where panicking on failure is preferred
    /// over terminating the process.
    #[allow(dead_code)]
    pub fn run_sync(&self) -> Result<Vec<EvalResult>> {
        if !self.fixtures_dir.exists() {
            return Err(Error::Config(format!(
                "Fixtures directory not found: {}",
                self.fixtures_dir.display()
            )));
        }

        let fixtures = self.discover_fixtures()?;
        let mut results = Vec::new();
        for fixture_dir in &fixtures {
            results.push(self.run_fixture(fixture_dir));
        }
        Ok(results)
    }

    pub async fn run(&self) -> Result<()> {
        if !self.fixtures_dir.exists() {
            return Err(Error::Config(format!(
                "Fixtures directory not found: {}",
                self.fixtures_dir.display()
            )));
        }

        let fixtures = self.discover_fixtures()?;

        if fixtures.is_empty() {
            eprintln!(
                "{} No fixtures found in {}",
                style("warning:").yellow().bold(),
                self.fixtures_dir.display()
            );
            return Ok(());
        }

        eprintln!(
            "{} Running {} evaluation fixture(s)...\n",
            style("eval:").cyan().bold(),
            fixtures.len()
        );

        let mut results = Vec::new();
        for fixture_dir in &fixtures {
            let result = self.run_fixture(fixture_dir);
            results.push(result);
        }

        self.print_results(&results);

        // Print aggregate summary
        let summary = EvalSummary::from_results(&results);
        eprintln!("{}", summary.format_report());

        if summary.total_failed > 0 {
            eprintln!(
                "{} {} fixture(s) failed",
                style("FAIL").red().bold(),
                summary.total_failed,
            );
            std::process::exit(1);
        }

        eprintln!(
            "{} All {} fixture(s) passed",
            style("PASS").green().bold(),
            summary.total_passed,
        );

        Ok(())
    }

    /// Discover fixture subdirectories, optionally filtered by pattern.
    fn discover_fixtures(&self) -> Result<Vec<PathBuf>> {
        let mut fixtures = Vec::new();

        let entries = std::fs::read_dir(&self.fixtures_dir).map_err(|e| {
            Error::Config(format!(
                "Cannot read fixtures directory {}: {}",
                self.fixtures_dir.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Must contain metadata.toml
            if !path.join("metadata.toml").exists() {
                continue;
            }

            // Apply filter if set
            if let Some(ref filter) = self.filter {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !dir_name.contains(filter.as_str()) {
                    continue;
                }
            }

            fixtures.push(path);
        }

        fixtures.sort();
        Ok(fixtures)
    }

    /// Run a single fixture through the pipeline.
    fn run_fixture(&self, fixture_dir: &Path) -> EvalResult {
        let dir_name = fixture_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Load metadata
        let metadata = match self.load_metadata(fixture_dir) {
            Ok(m) => m,
            Err(e) => {
                return EvalResult {
                    fixture_name: dir_name,
                    description: String::new(),
                    expected_type: String::new(),
                    actual_type: String::new(),
                    type_passed: false,
                    expected_scope: None,
                    actual_scope: None,
                    scope_passed: false,
                    prompt_assembled: false,
                    sanitizer_result: None,
                    message_check: None,
                    assertion_failures: Vec::new(),
                    error: Some(format!("Failed to load metadata: {}", e)),
                };
            }
        };

        // Load diff
        let diff_content = match std::fs::read_to_string(fixture_dir.join("diff.patch")) {
            Ok(d) => d,
            Err(e) => {
                return EvalResult {
                    fixture_name: metadata.name,
                    description: metadata.description,
                    expected_type: metadata.expected_type,
                    actual_type: String::new(),
                    type_passed: false,
                    expected_scope: metadata.expected_scope,
                    actual_scope: None,
                    scope_passed: false,
                    prompt_assembled: false,
                    sanitizer_result: None,
                    message_check: None,
                    assertion_failures: Vec::new(),
                    error: Some(format!("Failed to load diff.patch: {}", e)),
                };
            }
        };

        // Load optional config overrides
        let config = self.load_config(fixture_dir);

        // Parse diff into StagedChanges
        let changes = Self::parse_diff_to_changes(&diff_content);

        // Load optional symbols from symbols.toml
        let symbols = self.load_symbols(fixture_dir);

        // Run context builder with injected symbols
        let context = ContextBuilder::build(&changes, &symbols, &config);

        // Check type inference
        let actual_type = context.suggested_type.as_str().to_string();
        let type_passed = actual_type.eq_ignore_ascii_case(&metadata.expected_type);

        // Check scope inference
        let actual_scope = context.suggested_scope.clone();
        let scope_passed = match &metadata.expected_scope {
            Some(expected) if expected == "optional" => true, // Any scope is fine
            Some(expected) => actual_scope.as_deref() == Some(expected.as_str()),
            None => true, // No expectation
        };

        // Check prompt assembly
        let prompt = context.to_prompt();
        let prompt_assembled = !prompt.is_empty() && prompt.contains("SUMMARY:");

        // Run new assertion checks
        let mut assertion_failures = Vec::new();
        Self::check_evidence(&metadata, &context, &mut assertion_failures);
        Self::check_prompt_content(&metadata, &prompt, &mut assertion_failures);
        Self::check_connections(&metadata, &context, &mut assertion_failures);
        Self::check_breaking(&metadata, &context, &mut assertion_failures);

        // Check sanitizer if response.json exists
        let sanitizer_result = self.check_sanitizer(fixture_dir, &config);

        // Check subject assertions against sanitized message
        Self::check_subject(&metadata, &sanitizer_result, &mut assertion_failures);

        // Check expected message if expected.txt exists
        let message_check = self.check_expected_message(fixture_dir, &sanitizer_result);

        EvalResult {
            fixture_name: metadata.name,
            description: metadata.description,
            expected_type: metadata.expected_type,
            actual_type,
            type_passed,
            expected_scope: metadata.expected_scope,
            actual_scope,
            scope_passed,
            prompt_assembled,
            sanitizer_result,
            message_check,
            assertion_failures,
            error: None,
        }
    }

    fn load_metadata(&self, fixture_dir: &Path) -> Result<FixtureMetadata> {
        let content = std::fs::read_to_string(fixture_dir.join("metadata.toml"))
            .map_err(|e| Error::Config(format!("Cannot read metadata.toml: {}", e)))?;
        toml::from_str(&content).map_err(|e| Error::Config(format!("Invalid metadata.toml: {}", e)))
    }

    fn load_config(&self, fixture_dir: &Path) -> Config {
        let config_path = fixture_dir.join("config.toml");
        if config_path.exists()
            && let Ok(content) = std::fs::read_to_string(&config_path)
            && let Ok(config) = toml::from_str::<Config>(&content)
        {
            return config;
        }
        Config::default()
    }

    /// Load symbols from `symbols.toml` if it exists, converting to `Vec<CodeSymbol>`.
    fn load_symbols(&self, fixture_dir: &Path) -> Vec<CodeSymbol> {
        let symbols_path = fixture_dir.join("symbols.toml");
        if !symbols_path.exists() {
            return Vec::new();
        }

        let content = match std::fs::read_to_string(&symbols_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let symbols_file: SymbolsFile = match toml::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "{} Failed to parse symbols.toml: {}",
                    style("warning:").yellow().bold(),
                    e
                );
                return Vec::new();
            }
        };

        symbols_file
            .symbols
            .into_iter()
            .map(|def| CodeSymbol {
                kind: parse_symbol_kind(&def.kind),
                name: def.name,
                file: PathBuf::from(def.file),
                line: def.line,
                end_line: def.end_line,
                is_public: def.is_public,
                is_added: def.is_added,
                is_whitespace_only: def.is_whitespace_only,
                signature: def.signature,
                parent_scope: def.parent_scope,
            })
            .collect()
    }

    /// Check evidence flag assertions.
    fn check_evidence(
        metadata: &FixtureMetadata,
        context: &crate::domain::PromptContext,
        failures: &mut Vec<AssertionFailure>,
    ) {
        let Some(ref evidence) = metadata.evidence else {
            return;
        };

        if let Some(expected) = evidence.is_mechanical
            && context.is_mechanical != expected
        {
            failures.push(AssertionFailure {
                category: "evidence".to_string(),
                message: format!(
                    "is_mechanical: expected={}, actual={}",
                    expected, context.is_mechanical
                ),
            });
        }

        if let Some(expected) = evidence.has_bug_evidence
            && context.has_bug_evidence != expected
        {
            failures.push(AssertionFailure {
                category: "evidence".to_string(),
                message: format!(
                    "has_bug_evidence: expected={}, actual={}",
                    expected, context.has_bug_evidence
                ),
            });
        }

        if let Some(expected) = evidence.has_new_public_api
            && context.has_new_public_api != expected
        {
            failures.push(AssertionFailure {
                category: "evidence".to_string(),
                message: format!(
                    "has_new_public_api: expected={}, actual={}",
                    expected, context.has_new_public_api
                ),
            });
        }

        if let Some(expected) = evidence.public_api_removed_count
            && context.public_api_removed_count != expected
        {
            failures.push(AssertionFailure {
                category: "evidence".to_string(),
                message: format!(
                    "public_api_removed_count: expected={}, actual={}",
                    expected, context.public_api_removed_count
                ),
            });
        }

        if let Some(expected) = evidence.is_dependency_only
            && context.is_dependency_only != expected
        {
            failures.push(AssertionFailure {
                category: "evidence".to_string(),
                message: format!(
                    "is_dependency_only: expected={}, actual={}",
                    expected, context.is_dependency_only
                ),
            });
        }
    }

    /// Check prompt content assertions (must_contain / must_not_contain).
    fn check_prompt_content(
        metadata: &FixtureMetadata,
        prompt: &str,
        failures: &mut Vec<AssertionFailure>,
    ) {
        let Some(ref prompt_exp) = metadata.prompt else {
            return;
        };

        for pattern in &prompt_exp.must_contain {
            if !prompt.contains(pattern.as_str()) {
                failures.push(AssertionFailure {
                    category: "prompt".to_string(),
                    message: format!("must_contain not found: \"{}\"", pattern),
                });
            }
        }

        for pattern in &prompt_exp.must_not_contain {
            if prompt.contains(pattern.as_str()) {
                failures.push(AssertionFailure {
                    category: "prompt".to_string(),
                    message: format!("must_not_contain was found: \"{}\"", pattern),
                });
            }
        }
    }

    /// Check cross-file connection assertions.
    fn check_connections(
        metadata: &FixtureMetadata,
        context: &crate::domain::PromptContext,
        failures: &mut Vec<AssertionFailure>,
    ) {
        let Some(ref conn_exp) = metadata.connections else {
            return;
        };

        if let Some(expected_count) = conn_exp.expected_count
            && context.connections.len() != expected_count
        {
            failures.push(AssertionFailure {
                category: "connections".to_string(),
                message: format!(
                    "expected_count={}, actual={}",
                    expected_count,
                    context.connections.len()
                ),
            });
        }

        if let Some(min_count) = conn_exp.min_count
            && context.connections.len() < min_count
        {
            failures.push(AssertionFailure {
                category: "connections".to_string(),
                message: format!(
                    "min_count={}, actual={}",
                    min_count,
                    context.connections.len()
                ),
            });
        }

        let connections_text = context.connections.join(" ");
        for pattern in &conn_exp.must_contain {
            if !connections_text.contains(pattern.as_str()) {
                failures.push(AssertionFailure {
                    category: "connections".to_string(),
                    message: format!(
                        "must_contain not found: \"{}\"\n  actual connections: {:?}",
                        pattern, context.connections
                    ),
                });
            }
        }
    }

    /// Check breaking change assertions.
    fn check_breaking(
        metadata: &FixtureMetadata,
        context: &crate::domain::PromptContext,
        failures: &mut Vec<AssertionFailure>,
    ) {
        let Some(ref breaking_exp) = metadata.breaking else {
            return;
        };

        if let Some(expected) = breaking_exp.expected {
            let has_breaking_signals = !context.metadata_breaking_signals.is_empty()
                || context.public_api_removed_count > 0;

            if expected && !has_breaking_signals {
                failures.push(AssertionFailure {
                    category: "breaking".to_string(),
                    message: "expected breaking signals, but none detected".to_string(),
                });
            } else if !expected && has_breaking_signals {
                failures.push(AssertionFailure {
                    category: "breaking".to_string(),
                    message: format!(
                        "expected no breaking signals, but found: metadata_signals={:?}, public_api_removed={}",
                        context.metadata_breaking_signals, context.public_api_removed_count
                    ),
                });
            }
        }
    }

    /// Check subject line assertions against the sanitized message.
    fn check_subject(
        metadata: &FixtureMetadata,
        sanitizer_result: &Option<SanitizerCheck>,
        failures: &mut Vec<AssertionFailure>,
    ) {
        let Some(ref subject_exp) = metadata.subject else {
            return;
        };

        // Subject checks only apply if we have a sanitized message
        let Some(sanitizer) = sanitizer_result else {
            return;
        };
        let Some(ref message) = sanitizer.actual_message else {
            return;
        };

        // Extract subject (first line, after "type(scope): " prefix)
        let first_line = message.lines().next().unwrap_or("");

        // The subject is after ": " in conventional commits
        let subject = first_line
            .find(": ")
            .map(|i| &first_line[i + 2..])
            .unwrap_or(first_line);

        for pattern in &subject_exp.must_contain {
            let lower_subject = subject.to_lowercase();
            let lower_pattern = pattern.to_lowercase();
            if !lower_subject.contains(&lower_pattern) {
                failures.push(AssertionFailure {
                    category: "subject".to_string(),
                    message: format!(
                        "must_contain not found: \"{}\" in subject \"{}\"",
                        pattern, subject
                    ),
                });
            }
        }

        for pattern in &subject_exp.must_not_contain {
            let lower_subject = subject.to_lowercase();
            let lower_pattern = pattern.to_lowercase();
            if lower_subject.contains(&lower_pattern) {
                failures.push(AssertionFailure {
                    category: "subject".to_string(),
                    message: format!(
                        "must_not_contain was found: \"{}\" in subject \"{}\"",
                        pattern, subject
                    ),
                });
            }
        }

        if let Some(max_len) = subject_exp.max_length
            && first_line.len() > max_len
        {
            failures.push(AssertionFailure {
                category: "subject".to_string(),
                message: format!(
                    "first line length {} exceeds max_length {}",
                    first_line.len(),
                    max_len
                ),
            });
        }
    }

    fn check_sanitizer(&self, fixture_dir: &Path, config: &Config) -> Option<SanitizerCheck> {
        let response_path = fixture_dir.join("response.json");
        if !response_path.exists() {
            return None;
        }

        let raw_response = match std::fs::read_to_string(&response_path) {
            Ok(r) => r,
            Err(e) => {
                return Some(SanitizerCheck {
                    passed: false,
                    actual_message: None,
                    error: Some(format!("Failed to read response.json: {}", e)),
                });
            }
        };

        match CommitSanitizer::sanitize(&raw_response, &config.format) {
            Ok(message) => Some(SanitizerCheck {
                passed: true,
                actual_message: Some(message),
                error: None,
            }),
            Err(e) => Some(SanitizerCheck {
                passed: false,
                actual_message: None,
                error: Some(format!("Sanitizer failed: {}", e)),
            }),
        }
    }

    fn check_expected_message(
        &self,
        fixture_dir: &Path,
        sanitizer_result: &Option<SanitizerCheck>,
    ) -> Option<MessageCheck> {
        let expected_path = fixture_dir.join("expected.txt");
        if !expected_path.exists() {
            return None;
        }

        let expected_content = match std::fs::read_to_string(&expected_path) {
            Ok(c) => c,
            Err(_) => return None,
        };

        let expected_first_line = expected_content
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        let actual_first_line = sanitizer_result
            .as_ref()
            .and_then(|s| s.actual_message.as_ref())
            .and_then(|m| m.lines().next())
            .map(|l| l.to_string());

        let passed = actual_first_line
            .as_deref()
            .is_some_and(|actual| actual == expected_first_line);

        Some(MessageCheck {
            expected_first_line,
            actual_first_line,
            passed,
        })
    }

    /// Parse a unified diff into `StagedChanges` for evaluation.
    ///
    /// This is a simplified parser that extracts file paths, status,
    /// and line counts from a standard unified diff format.
    fn parse_diff_to_changes(diff_content: &str) -> StagedChanges {
        let mut files: Vec<FileChange> = Vec::new();
        let mut current_path: Option<PathBuf> = None;
        let mut current_diff = String::new();
        let mut additions: usize = 0;
        let mut deletions: usize = 0;
        let mut is_new_file = false;
        let mut is_deleted_file = false;

        for line in diff_content.lines() {
            if line.starts_with("diff --git") {
                // Flush previous file
                if let Some(path) = current_path.take() {
                    let status = if is_new_file {
                        ChangeStatus::Added
                    } else if is_deleted_file {
                        ChangeStatus::Deleted
                    } else {
                        ChangeStatus::Modified
                    };

                    files.push(FileChange {
                        category: FileCategory::from_path(&path),
                        path,
                        status,
                        diff: Arc::from(current_diff.as_str()),
                        additions,
                        deletions,
                        is_binary: false,
                        old_path: None,
                        rename_similarity: None,
                    });
                }

                // Parse new file path from "diff --git a/path b/path"
                current_path = line.split(" b/").nth(1).map(|p| PathBuf::from(p.trim()));
                current_diff = String::new();
                additions = 0;
                deletions = 0;
                is_new_file = false;
                is_deleted_file = false;
            } else if line.starts_with("new file mode") {
                is_new_file = true;
            } else if line.starts_with("deleted file mode") {
                is_deleted_file = true;
            } else if line.starts_with('+') && !line.starts_with("+++") {
                additions += 1;
                current_diff.push_str(line);
                current_diff.push('\n');
            } else if line.starts_with('-') && !line.starts_with("---") {
                deletions += 1;
                current_diff.push_str(line);
                current_diff.push('\n');
            } else if line.starts_with(' ') || line.starts_with("@@") {
                current_diff.push_str(line);
                current_diff.push('\n');
            }
        }

        // Flush last file
        if let Some(path) = current_path {
            let status = if is_new_file {
                ChangeStatus::Added
            } else if is_deleted_file {
                ChangeStatus::Deleted
            } else {
                ChangeStatus::Modified
            };

            files.push(FileChange {
                category: FileCategory::from_path(&path),
                path,
                status,
                diff: Arc::from(current_diff.as_str()),
                additions,
                deletions,
                is_binary: false,
                old_path: None,
                rename_similarity: None,
            });
        }

        let stats = DiffStats {
            files_changed: files.len(),
            insertions: files.iter().map(|f| f.additions).sum(),
            deletions: files.iter().map(|f| f.deletions).sum(),
        };

        StagedChanges { files, stats }
    }

    fn print_results(&self, results: &[EvalResult]) {
        for result in results {
            let status = if result.passed() {
                style("PASS").green().bold()
            } else {
                style("FAIL").red().bold()
            };

            eprintln!(
                "  [{}] {} — {}",
                status, result.fixture_name, result.description
            );

            // Type inference
            let type_icon = if result.type_passed { "ok" } else { "MISMATCH" };
            eprintln!(
                "    Type: expected={}, actual={} [{}]",
                result.expected_type, result.actual_type, type_icon
            );

            // Scope inference (if expected)
            if let Some(ref expected_scope) = result.expected_scope {
                let scope_icon = if result.scope_passed {
                    "ok"
                } else {
                    "MISMATCH"
                };
                eprintln!(
                    "    Scope: expected={}, actual={} [{}]",
                    expected_scope,
                    result.actual_scope.as_deref().unwrap_or("none"),
                    scope_icon
                );
            }

            // Prompt assembly
            let prompt_icon = if result.prompt_assembled {
                "ok"
            } else {
                "FAIL"
            };
            eprintln!("    Prompt: [{}]", prompt_icon);

            // Assertion failures
            for failure in &result.assertion_failures {
                eprintln!("    {} {}", style("FAIL").red(), failure);
            }

            // Sanitizer check
            if let Some(ref sanitizer) = result.sanitizer_result {
                let san_icon = if sanitizer.passed { "ok" } else { "FAIL" };
                eprintln!("    Sanitizer: [{}]", san_icon);
                if let Some(ref msg) = sanitizer.actual_message {
                    let first_line = msg.lines().next().unwrap_or("");
                    eprintln!("      Output: {}", first_line);
                }
                if let Some(ref err) = sanitizer.error {
                    eprintln!("      Error: {}", err);
                }
            }

            // Message check
            if let Some(ref msg_check) = result.message_check {
                let msg_icon = if msg_check.passed { "ok" } else { "MISMATCH" };
                eprintln!("    Message: [{}]", msg_icon);
                if !msg_check.passed {
                    eprintln!("      Expected: {}", msg_check.expected_first_line);
                    eprintln!(
                        "      Actual:   {}",
                        msg_check.actual_first_line.as_deref().unwrap_or("(none)")
                    );
                }
            }

            // Error
            if let Some(ref err) = result.error {
                eprintln!("    Error: {}", err);
            }

            eprintln!();
        }
    }
}

/// Aggregate evaluation summary with per-type accuracy breakdown.
#[derive(Debug)]
pub struct EvalSummary {
    pub total_fixtures: usize,
    pub total_passed: usize,
    pub total_failed: usize,
    /// Per-type accuracy: (type_name, passed, total).
    pub per_type: Vec<(String, usize, usize)>,
}

impl EvalSummary {
    /// Build a summary from eval results.
    #[must_use]
    pub fn from_results(results: &[EvalResult]) -> Self {
        let total_fixtures = results.len();
        let total_passed = results.iter().filter(|r| r.passed()).count();
        let total_failed = total_fixtures - total_passed;

        // Group by expected_type
        let mut type_map: std::collections::BTreeMap<String, (usize, usize)> =
            std::collections::BTreeMap::new();

        for result in results {
            let key = result.expected_type.to_lowercase();
            if key.is_empty() {
                continue;
            }
            let entry = type_map.entry(key).or_insert((0, 0));
            entry.1 += 1; // total
            if result.passed() {
                entry.0 += 1; // passed
            }
        }

        let per_type: Vec<(String, usize, usize)> = type_map
            .into_iter()
            .map(|(k, (passed, total))| (k, passed, total))
            .collect();

        Self {
            total_fixtures,
            total_passed,
            total_failed,
            per_type,
        }
    }

    /// Format the summary as a human-readable report.
    #[must_use]
    pub fn format_report(&self) -> String {
        let mut report = String::new();

        report.push_str("=== Eval Summary ===\n\n");

        // Per-type breakdown
        report.push_str("Per-type accuracy:\n");
        for (type_name, passed, total) in &self.per_type {
            let pct = if *total > 0 {
                (*passed as f64 / *total as f64) * 100.0
            } else {
                0.0
            };
            report.push_str(&format!(
                "  {}: {}/{} ({:.0}%)\n",
                type_name, passed, total, pct
            ));
        }

        // Overall score
        let overall_pct = if self.total_fixtures > 0 {
            (self.total_passed as f64 / self.total_fixtures as f64) * 100.0
        } else {
            0.0
        };
        report.push_str(&format!(
            "\nOverall: {}/{} ({:.1}%)\n",
            self.total_passed, self.total_fixtures, overall_pct
        ));

        report
    }
}

/// Parse a symbol kind string from TOML into `SymbolKind`.
fn parse_symbol_kind(kind: &str) -> SymbolKind {
    match kind.to_lowercase().as_str() {
        "function" | "fn" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "struct" => SymbolKind::Struct,
        "enum" => SymbolKind::Enum,
        "trait" => SymbolKind::Trait,
        "impl" => SymbolKind::Impl,
        "class" => SymbolKind::Class,
        "interface" => SymbolKind::Interface,
        "const" | "constant" => SymbolKind::Const,
        "type" => SymbolKind::Type,
        _ => SymbolKind::Function, // Default fallback
    }
}
