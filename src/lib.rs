// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

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
