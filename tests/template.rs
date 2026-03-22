// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Tests for the template engine (custom prompt templates).

use std::collections::HashMap;
use std::path::PathBuf;

use commitbee::services::template;

// ─── Template loading and variable substitution ─────────────────────────────

#[test]
fn render_template_substitutes_variables() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("template.txt");
    std::fs::write(
        &path,
        "Diff:\n{{diff}}\n\nFiles: {{files}}\nSymbols: {{symbols}}",
    )
    .unwrap();

    let mut vars = HashMap::new();
    vars.insert("diff", "+fn foo() {}");
    vars.insert("files", "src/main.rs");
    vars.insert("symbols", "Function foo (added)");

    let result = template::render_template(&path, &vars).unwrap();

    assert_eq!(
        result,
        "Diff:\n+fn foo() {}\n\nFiles: src/main.rs\nSymbols: Function foo (added)"
    );
}

// ─── Missing file produces clear error ──────────────────────────────────────

#[test]
fn render_template_missing_file_returns_error() {
    let path = PathBuf::from("/nonexistent/template.txt");
    let vars = HashMap::new();

    let result = template::render_template(&path, &vars);

    assert!(result.is_err(), "expected error for missing file");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("failed to read template file"),
        "expected descriptive error, got: {err_msg}"
    );
    assert!(
        err_msg.contains("/nonexistent/template.txt"),
        "expected path in error, got: {err_msg}"
    );
}

// ─── Unknown variables left as-is ───────────────────────────────────────────

#[test]
fn render_template_unknown_variables_left_intact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("template.txt");
    std::fs::write(&path, "Known: {{diff}}\nUnknown: {{future_var}}").unwrap();

    let mut vars = HashMap::new();
    vars.insert("diff", "the diff");

    let result = template::render_template(&path, &vars).unwrap();

    assert_eq!(result, "Known: the diff\nUnknown: {{future_var}}");
}

// ─── Empty template file works ──────────────────────────────────────────────

#[test]
fn render_template_empty_file_returns_empty_string() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.txt");
    std::fs::write(&path, "").unwrap();

    let vars = HashMap::new();

    let result = template::render_template(&path, &vars).unwrap();
    assert_eq!(result, "");
}

// ─── load_file works ────────────────────────────────────────────────────────

#[test]
fn load_file_reads_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prompt.txt");
    std::fs::write(&path, "You are a helpful assistant.").unwrap();

    let result = template::load_file(&path).unwrap();
    assert_eq!(result, "You are a helpful assistant.");
}

#[test]
fn load_file_missing_returns_error() {
    let path = PathBuf::from("/nonexistent/prompt.txt");

    let result = template::load_file(&path);

    assert!(result.is_err(), "expected error for missing file");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("failed to read prompt file"),
        "expected descriptive error, got: {err_msg}"
    );
}

// ─── All supported variables ────────────────────────────────────────────────

#[test]
fn render_template_all_supported_variables() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("full.txt");
    std::fs::write(
        &path,
        "Type: {{type}}\nScope: {{scope}}\nDiff: {{diff}}\nFiles: {{files}}\nSymbols: {{symbols}}\nEvidence: {{evidence}}\nConstraints: {{constraints}}",
    )
    .unwrap();

    let mut vars = HashMap::new();
    vars.insert("type", "feat");
    vars.insert("scope", "cli");
    vars.insert("diff", "+new code");
    vars.insert("files", "src/cli.rs");
    vars.insert("symbols", "fn main");
    vars.insert("evidence", "has bug evidence: no");
    vars.insert("constraints", "no constraints");

    let result = template::render_template(&path, &vars).unwrap();

    assert!(result.contains("Type: feat"));
    assert!(result.contains("Scope: cli"));
    assert!(result.contains("Diff: +new code"));
    assert!(result.contains("Files: src/cli.rs"));
    assert!(result.contains("Symbols: fn main"));
    assert!(result.contains("Evidence: has bug evidence: no"));
    assert!(result.contains("Constraints: no constraints"));
}
