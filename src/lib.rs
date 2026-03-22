// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

#![forbid(unsafe_code)]

//! CommitBee - AI-powered commit message generator
//!
//! This crate provides the core functionality for analyzing git changes
//! using tree-sitter and generating conventional commit messages via LLMs.

pub mod app;
pub mod cli;
pub mod config;
pub mod domain;
pub mod error;
#[cfg(feature = "eval")]
pub mod eval;
pub mod services;

pub use app::App;
pub use cli::Cli;
pub use config::Config;
pub use error::{Error, Result};

// ── Thin wrappers for fuzz targets ──

/// Sanitize a raw LLM response into a conventional commit message.
///
/// Wrapper around `CommitSanitizer::sanitize` for fuzz target access.
/// Returns `Ok(message)` on valid input, `Err` on invalid commit format.
pub fn sanitize_commit_message(
    raw: &str,
    include_body: bool,
    include_scope: bool,
) -> Result<String> {
    let format = config::CommitFormat {
        include_body,
        include_scope,
        lowercase_subject: true,
    };
    services::sanitizer::CommitSanitizer::sanitize(raw, &format)
}

/// Scan a full unified diff for leaked secrets using default patterns.
///
/// Wrapper around `safety::scan_full_diff_for_secrets` for fuzz target access.
pub fn scan_full_diff_for_secrets(diff: &str) -> Vec<services::safety::SecretMatch> {
    services::safety::scan_full_diff_for_secrets(diff)
}

/// Parse unified diff hunk headers into structured `DiffHunk` values.
///
/// Wrapper around `DiffHunk::parse_from_diff` for fuzz target access.
pub fn parse_diff_hunks(diff: &str) -> Vec<services::analyzer::DiffHunk> {
    services::analyzer::DiffHunk::parse_from_diff(diff)
}

/// Extract signature from Rust source code for fuzz target access.
///
/// Parses the source with tree-sitter Rust, finds the first top-level definition,
/// and extracts its signature. Must never panic on any input.
#[cfg(feature = "lang-rust")]
pub fn extract_rust_signature(source: &str) -> Option<String> {
    use tree_sitter::Parser;
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .is_err()
    {
        return None;
    }
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();
    let first_child = root.child(0)?;
    services::analyzer::AnalyzerService::extract_signature(first_child, source)
}

/// Classify whether a diff span contains whitespace-only changes for fuzz target access.
///
/// Wrapper around `ContextBuilder::classify_span_change`. Must never panic on any input.
pub fn classify_diff_span(
    diff: &str,
    new_start: usize,
    new_end: usize,
    old_start: usize,
    old_end: usize,
) -> Option<bool> {
    services::context::ContextBuilder::classify_span_change(
        diff, new_start, new_end, old_start, old_end,
    )
}
