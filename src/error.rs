// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("No staged changes found. Use `git add` to stage files.")]
    NoStagedChanges,

    #[error("Not a git repository. Run from a git project directory.")]
    NotAGitRepo,

    #[error("Repository has no commits. Make an initial commit first.")]
    EmptyRepository,

    #[error("Merge conflicts detected. Resolve conflicts before committing.")]
    MergeConflicts,

    #[error("Merge in progress. Complete or abort the merge first.")]
    MergeInProgress,

    #[error("Operation cancelled by user.")]
    Cancelled,

    #[error("Potential secrets detected: {patterns:?}. Use --allow-secrets to proceed.")]
    SecretsDetected { patterns: Vec<String> },

    #[error("Provider '{provider}' error: {message}")]
    Provider { provider: String, message: String },

    #[error("Invalid commit message: {0}")]
    InvalidCommitMessage(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error("Dialog error: {0}")]
    Dialog(String),
}

impl From<dialoguer::Error> for Error {
    fn from(e: dialoguer::Error) -> Self {
        Error::Dialog(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
