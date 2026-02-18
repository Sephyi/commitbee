// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

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

    /// Generate N candidate messages (default 1, max 5)
    #[arg(short = 'n', long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=5))]
    pub generate: u8,

    /// Show the prompt sent to LLM
    #[arg(long)]
    pub show_prompt: bool,

    /// Disable commit split suggestions
    #[arg(long)]
    pub no_split: bool,

    /// Disable scope in commit messages
    #[arg(long)]
    pub no_scope: bool,

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
