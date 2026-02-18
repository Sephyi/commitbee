// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// miette's Diagnostic derive generates code that triggers this false positive
#![allow(unused_assignments)]

use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum Error {
    #[error("No staged changes found")]
    #[diagnostic(
        code(commitbee::git::no_staged),
        help("Stage files with: git add <files>")
    )]
    NoStagedChanges,

    #[error("Not a git repository")]
    #[diagnostic(
        code(commitbee::git::not_repo),
        help("Run this command inside a git repository")
    )]
    NotAGitRepo,

    #[error("Merge conflicts detected")]
    #[diagnostic(
        code(commitbee::git::conflicts),
        help("Resolve conflicts before committing")
    )]
    MergeConflicts,

    #[error("Merge in progress")]
    #[diagnostic(
        code(commitbee::git::merge),
        help("Complete or abort the merge: git merge --abort")
    )]
    MergeInProgress,

    #[error("Operation cancelled by user")]
    Cancelled,

    #[error("Potential secrets detected: {patterns:?}")]
    #[diagnostic(
        code(commitbee::safety::secrets),
        help("Use --allow-secrets with local Ollama only")
    )]
    SecretsDetected { patterns: Vec<String> },

    #[error("Commit split aborted: files with both staged and unstaged changes")]
    #[diagnostic(
        code(commitbee::split::aborted),
        help("Stash or commit unstaged changes first, or use --no-split")
    )]
    SplitAborted,

    #[error("Cannot connect to Ollama at {host}")]
    #[diagnostic(
        code(commitbee::ollama::not_running),
        help("Start Ollama with: ollama serve")
    )]
    OllamaNotRunning { host: String },

    #[error("Model '{model}' not found. Available: {}", available.join(", "))]
    #[diagnostic(
        code(commitbee::ollama::model_not_found),
        help("Pull the model with: ollama pull {model}")
    )]
    ModelNotFound {
        model: String,
        available: Vec<String>,
    },

    #[error("Provider '{provider}' error: {message}")]
    #[diagnostic(code(commitbee::provider::error))]
    Provider { provider: String, message: String },

    #[error("Invalid commit message: {0}")]
    #[diagnostic(code(commitbee::commit::invalid))]
    InvalidCommitMessage(String),

    #[error("Configuration error: {0}")]
    #[diagnostic(code(commitbee::config::error))]
    Config(String),

    #[error("Git error: {0}")]
    #[diagnostic(code(commitbee::git::error))]
    Git(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error("Dialog error: {0}")]
    Dialog(String),

    #[cfg(feature = "secure-storage")]
    #[error("Keyring error: {0}")]
    #[diagnostic(
        code(commitbee::keyring::error),
        help("Check your system keychain configuration")
    )]
    Keyring(String),
}

impl From<dialoguer::Error> for Error {
    fn from(e: dialoguer::Error) -> Self {
        Error::Dialog(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
