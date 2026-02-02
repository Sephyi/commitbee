// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
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
pub enum Commands {
    /// Initialize config file
    Init,
    /// Show current configuration
    Config,
}
