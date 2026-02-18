// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "commitbee")]
#[command(version)]
#[command(about = "AI-powered commit message generator", long_about = None)]
pub struct Cli {
    /// LLM provider (ollama, openai, anthropic)
    #[arg(short, long, env = "COMMITBEE_PROVIDER")]
    pub provider: Option<String>,

    /// Model name
    #[arg(short, long, env = "COMMITBEE_MODEL")]
    pub model: Option<String>,

    /// Auto-confirm and commit without prompting
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Print message only, don't commit
    #[arg(long)]
    pub dry_run: bool,

    /// Allow committing with detected secrets (local only)
    #[arg(long)]
    pub allow_secrets: bool,

    /// Show the prompt sent to LLM
    #[arg(long)]
    pub show_prompt: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug)]
pub enum HookAction {
    /// Install prepare-commit-msg hook
    Install,
    /// Remove prepare-commit-msg hook
    Uninstall,
    /// Check if hook is installed
    Status,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Initialize config file
    Init,
    /// Show current configuration
    Config,
    /// Check configuration and provider connectivity
    Doctor,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Manage prepare-commit-msg git hook
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Store API key in system keychain
    #[cfg(feature = "secure-storage")]
    SetKey {
        /// Provider to store key for (openai, anthropic)
        provider: String,
    },
    /// Check if API key exists in system keychain
    #[cfg(feature = "secure-storage")]
    GetKey {
        /// Provider to check key for (openai, anthropic)
        provider: String,
    },
}
