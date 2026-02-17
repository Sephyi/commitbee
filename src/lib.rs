// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

//! CommitBee - AI-powered commit message generator
//!
//! This crate provides the core functionality for analyzing git changes
//! using tree-sitter and generating conventional commit messages via LLMs.

pub mod app;
pub mod cli;
pub mod config;
pub mod domain;
pub mod error;
pub mod services;

pub use app::App;
pub use cli::Cli;
pub use config::Config;
pub use error::{Error, Result};
