// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! Evaluation harness for pipeline quality testing.
//!
//! Runs fixture diffs through the context builder, commit type inference,
//! and sanitizer pipeline, then compares against expected output snapshots.
//! Feature-gated behind `eval`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use console::style;
use serde::Deserialize;

use crate::config::Config;
use crate::domain::{ChangeStatus, CodeSymbol, DiffStats, FileCategory, FileChange, StagedChanges};
use crate::error::{Error, Result};
use crate::services::context::ContextBuilder;
use crate::services::sanitizer::CommitSanitizer;

/// Metadata describing a single evaluation fixture.
#[derive(Debug, Deserialize)]
struct FixtureMetadata {
    name: String,
    description: String,
    expected_type: String,
    #[serde(default)]
    expected_scope: Option<String>,
}

/// Result of running one fixture through the pipeline.
#[derive(Debug)]
struct EvalResult {
    fixture_name: String,
    description: String,
    // Type inference check
    expected_type: String,
    actual_type: String,
    type_passed: bool,
    // Scope check (only if expected_scope is set)
    expected_scope: Option<String>,
    actual_scope: Option<String>,
    scope_passed: bool,
    // Prompt assembly check
    prompt_assembled: bool,
    // Sanitizer check (only if response.json exists)
    sanitizer_result: Option<SanitizerCheck>,
    // Expected message check (only if expected.txt exists)
    message_check: Option<MessageCheck>,
    // Overall
    error: Option<String>,
}

#[derive(Debug)]
struct SanitizerCheck {
    passed: bool,
    actual_message: Option<String>,
    error: Option<String>,
}

#[derive(Debug)]
struct MessageCheck {
    expected_first_line: String,
    actual_first_line: Option<String>,
    passed: bool,
}

impl EvalResult {
    fn passed(&self) -> bool {
        self.type_passed
            && self.scope_passed
            && self.prompt_assembled
            && self.error.is_none()
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

        let failed = results.iter().filter(|r| !r.passed()).count();
        if failed > 0 {
            eprintln!(
                "\n{} {} fixture(s) failed",
                style("FAIL").red().bold(),
                failed,
            );
            std::process::exit(1);
        }

        eprintln!(
            "\n{} All {} fixture(s) passed",
            style("PASS").green().bold(),
            results.len(),
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
                    error: Some(format!("Failed to load diff.patch: {}", e)),
                };
            }
        };

        // Load optional config overrides
        let config = self.load_config(fixture_dir);

        // Parse diff into StagedChanges
        let changes = Self::parse_diff_to_changes(&diff_content);

        // Run context builder (no tree-sitter — we don't have actual files)
        let symbols: Vec<CodeSymbol> = Vec::new();
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

        // Check sanitizer if response.json exists
        let sanitizer_result = self.check_sanitizer(fixture_dir, &config);

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
