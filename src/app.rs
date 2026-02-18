// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;

use console::style;
use dialoguer::Confirm;
use tokio::signal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::cli::{Cli, Commands};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::services::{
    analyzer::AnalyzerService, context::ContextBuilder, git::GitService, llm, safety,
    sanitizer::CommitSanitizer,
};

pub struct App {
    cli: Cli,
    config: Config,
    cancel_token: CancellationToken,
}

impl App {
    pub fn new(cli: Cli) -> Result<Self> {
        let config = Config::load(&cli)?;
        debug!(
            provider = %config.provider,
            model = %config.model,
            max_diff_lines = config.max_diff_lines,
            "config loaded"
        );
        let cancel_token = CancellationToken::new();
        Ok(Self {
            cli,
            config,
            cancel_token,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // Setup Ctrl+C handler with CancellationToken
        let cancel = self.cancel_token.clone();
        tokio::spawn(async move {
            signal::ctrl_c().await.ok();
            cancel.cancel();
        });

        // Handle subcommands
        if let Some(ref cmd) = self.cli.command {
            return self.handle_command(cmd).await;
        }

        self.generate_commit().await
    }

    async fn generate_commit(&mut self) -> Result<()> {
        if self.cancel_token.is_cancelled() {
            return Err(Error::Cancelled);
        }

        // Step 1: Discover repo and get changes
        self.print_status("Analyzing staged changes...");

        let git = GitService::discover()?;
        let changes = git.get_staged_changes(self.config.max_file_lines).await?;

        self.print_info(&format!(
            "{} files with changes detected (+{} -{})",
            changes.files.len(),
            changes.stats.insertions,
            changes.stats.deletions
        ));

        // Step 2: Check for safety issues
        if safety::check_for_conflicts(&changes) {
            return Err(Error::MergeConflicts);
        }

        let secrets = safety::scan_for_secrets(&changes);
        if !secrets.is_empty() && !self.cli.allow_secrets {
            warn!(
                count = secrets.len(),
                "potential secrets detected in staged changes"
            );
            self.print_warning("Potential secrets detected:");
            for s in &secrets {
                eprintln!(
                    "  {} in {} (line ~{})",
                    s.pattern_name,
                    s.file,
                    s.line.unwrap_or(0)
                );
            }

            if self.config.provider != crate::config::Provider::Ollama {
                return Err(Error::SecretsDetected {
                    patterns: secrets.iter().map(|s| s.pattern_name.clone()).collect(),
                });
            }
            self.print_info("Proceeding with local Ollama (data stays local)");
        }

        if self.cancel_token.is_cancelled() {
            return Err(Error::Cancelled);
        }

        // Step 3: Pre-fetch file content and analyze with tree-sitter
        self.print_status("Extracting code symbols...");

        let mut analyzer = AnalyzerService::new()?;

        // Pre-fetch all file content asynchronously, then pass as sync maps
        let file_paths: Vec<PathBuf> = changes.files.iter().map(|f| f.path.clone()).collect();
        let mut staged_map: HashMap<PathBuf, String> = HashMap::new();
        let mut head_map: HashMap<PathBuf, String> = HashMap::new();

        for path in &file_paths {
            if let Some(content) = git.get_staged_content(path).await {
                staged_map.insert(path.clone(), content);
            }
            if let Some(content) = git.get_head_content(path).await {
                head_map.insert(path.clone(), content);
            }
        }

        let symbols = analyzer.extract_symbols(
            &changes.files,
            &|path| staged_map.get(path).cloned(),
            &|path| head_map.get(path).cloned(),
        );

        debug!(count = symbols.len(), "symbols extracted");

        // Step 4: Build context
        let context = ContextBuilder::build(&changes, &symbols, &self.config);
        debug!(prompt_chars = context.to_prompt().len(), "context built");

        let prompt = context.to_prompt();

        if self.cli.show_prompt {
            eprintln!("{}", style("--- PROMPT ---").dim());
            eprintln!("{}", prompt);
            eprintln!("{}", style("--- END PROMPT ---").dim());
        }

        if self.cancel_token.is_cancelled() {
            return Err(Error::Cancelled);
        }

        // Step 5: Generate commit message
        self.print_status(&format!(
            "Contacting {} ({})...",
            self.config.provider, self.config.model
        ));

        let provider = llm::create_provider(&self.config)?;
        debug!(provider = provider.name(), "verifying provider");
        provider.verify().await?;

        // Setup streaming output
        let (tx, mut rx) = mpsc::channel::<String>(64);

        // Print streaming tokens (cancellable)
        let cancel_for_printer = self.cancel_token.clone();
        let print_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_for_printer.cancelled() => break,
                    token = rx.recv() => {
                        match token {
                            Some(t) => eprint!("{}", t),
                            None => break,
                        }
                    }
                }
            }
        });

        eprintln!("{} Generating...\n", style("info:").cyan());

        let raw_message = provider
            .generate(&prompt, tx, self.cancel_token.clone())
            .await?;

        // Wait for printer to finish
        let _ = print_handle.await;

        eprintln!(); // Newline after streaming

        if raw_message.trim().is_empty() {
            return Err(Error::Provider {
                provider: provider.name().into(),
                message: "Empty response received".into(),
            });
        }

        // Step 6: Sanitize and validate the commit message
        debug!(raw_len = raw_message.len(), "sanitizing LLM response");
        let message = CommitSanitizer::sanitize(&raw_message, &self.config.format)?;

        // Step 7: Confirm and commit
        if self.cli.dry_run {
            println!("\n{}", message);
            return Ok(());
        }

        // TTY detection for git hook compatibility
        let is_interactive = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();

        if !self.cli.yes {
            if !is_interactive {
                eprintln!("{}", style("warning:").yellow().bold());
                eprintln!("  Not a terminal. Use --yes to auto-confirm in scripts/hooks.");
                println!("\n{}", message);
                return Ok(());
            }

            eprintln!("\n{}", style("Generated commit message:").bold());
            eprintln!("{}", style(&message).green());
            eprintln!();

            let confirm = Confirm::new()
                .with_prompt("Create commit with this message?")
                .default(true)
                .interact()?;

            if !confirm {
                return Err(Error::Cancelled);
            }
        }

        // Create commit
        git.commit(&message).await?;

        eprintln!("{} Committed!", style("✓").green().bold());

        Ok(())
    }

    async fn handle_command(&self, cmd: &Commands) -> Result<()> {
        match cmd {
            Commands::Init => {
                let path = Config::create_default()?;
                println!("Created config: {}", path.display());
                Ok(())
            }
            Commands::Config => {
                println!("Provider: {}", self.config.provider);
                println!("Model: {}", self.config.model);
                println!("Ollama host: {}", self.config.ollama_host);
                println!("Max diff lines: {}", self.config.max_diff_lines);
                println!("Max file lines: {}", self.config.max_file_lines);
                println!("Max context chars: {}", self.config.max_context_chars);
                println!("Timeout: {}s", self.config.timeout_secs);
                println!("Temperature: {}", self.config.temperature);
                println!("Max tokens: {}", self.config.num_predict);
                println!();
                println!("[format]");
                println!("  include_body: {}", self.config.format.include_body);
                println!("  include_scope: {}", self.config.format.include_scope);
                println!(
                    "  lowercase_subject: {}",
                    self.config.format.lowercase_subject
                );
                Ok(())
            }
            Commands::Doctor => self.run_doctor().await,
            Commands::Completions { shell } => {
                let mut cmd = <Cli as clap::CommandFactory>::command();
                clap_complete::generate(*shell, &mut cmd, "commitbee", &mut std::io::stdout());
                Ok(())
            }
            #[cfg(feature = "secure-storage")]
            Commands::SetKey { provider } => self.set_api_key(provider),
            #[cfg(feature = "secure-storage")]
            Commands::GetKey { provider } => self.get_api_key(provider),
        }
    }

    async fn run_doctor(&self) -> Result<()> {
        eprintln!("{} Running diagnostics...\n", style("→").cyan());

        // Config summary
        eprintln!("{}", style("Configuration").bold().underlined());
        eprintln!("  Provider:    {}", self.config.provider);
        eprintln!("  Model:       {}", self.config.model);
        eprintln!("  Timeout:     {}s", self.config.timeout_secs);
        if let Some(ref path) = Config::config_path() {
            let status = if path.exists() { "found" } else { "not found" };
            eprintln!("  Config file: {} ({})", path.display(), status);
        }
        eprintln!();

        // Provider connectivity
        eprintln!("{}", style("Provider Check").bold().underlined());
        match self.config.provider {
            crate::config::Provider::Ollama => {
                eprint!("  Ollama ({}): ", self.config.ollama_host);
                let provider = llm::create_provider(&self.config)?;
                match provider.verify().await {
                    Ok(()) => {
                        eprintln!("{}", style("OK").green().bold());
                        eprintln!(
                            "  Model '{}': {}",
                            self.config.model,
                            style("available").green()
                        );
                    }
                    Err(Error::OllamaNotRunning { .. }) => {
                        eprintln!("{}", style("NOT RUNNING").red().bold());
                        eprintln!("  Start with: {}", style("ollama serve").yellow());
                    }
                    Err(Error::ModelNotFound { ref available, .. }) => {
                        eprintln!("{}", style("connected").green());
                        eprintln!(
                            "  Model '{}': {}",
                            self.config.model,
                            style("NOT FOUND").red().bold()
                        );
                        eprintln!(
                            "  Pull with: {}",
                            style(format!("ollama pull {}", self.config.model)).yellow()
                        );
                        if !available.is_empty() {
                            eprintln!("  Available: {}", available.join(", "));
                        }
                    }
                    Err(e) => {
                        eprintln!("{}: {}", style("ERROR").red().bold(), e);
                    }
                }
            }
            other => {
                eprint!("  {} API key: ", other);
                if self.config.api_key.is_some() {
                    eprintln!("{}", style("configured").green());
                } else {
                    eprintln!("{}", style("MISSING").red().bold());
                }
            }
        }
        eprintln!();

        // Git check
        eprintln!("{}", style("Git Repository").bold().underlined());
        match GitService::discover() {
            Ok(_) => eprintln!("  Repository: {}", style("found").green()),
            Err(_) => eprintln!("  Repository: {}", style("NOT FOUND").red().bold()),
        }

        eprintln!();
        eprintln!("{} Diagnostics complete.", style("✓").green().bold());

        Ok(())
    }

    // ─── Keyring Commands ───

    #[cfg(feature = "secure-storage")]
    fn set_api_key(&self, provider: &str) -> Result<()> {
        let provider_lower = provider.to_lowercase();
        if provider_lower != "openai" && provider_lower != "anthropic" {
            return Err(Error::Config(format!(
                "Keyring storage is only for cloud providers (openai, anthropic), got '{}'",
                provider
            )));
        }

        eprintln!(
            "Enter API key for {} (input will be hidden):",
            style(&provider_lower).bold()
        );

        let key = dialoguer::Password::new()
            .with_prompt("API key")
            .interact()
            .map_err(|e| Error::Dialog(e.to_string()))?;

        if key.trim().is_empty() {
            return Err(Error::Config("API key cannot be empty".into()));
        }

        let entry = keyring::Entry::new("commitbee", &provider_lower)
            .map_err(|e| Error::Keyring(e.to_string()))?;
        entry
            .set_password(&key)
            .map_err(|e| Error::Keyring(e.to_string()))?;

        eprintln!(
            "{} API key stored for {}",
            style("✓").green().bold(),
            provider_lower
        );
        Ok(())
    }

    #[cfg(feature = "secure-storage")]
    fn get_api_key(&self, provider: &str) -> Result<()> {
        let provider_lower = provider.to_lowercase();
        if provider_lower != "openai" && provider_lower != "anthropic" {
            return Err(Error::Config(format!(
                "Keyring storage is only for cloud providers (openai, anthropic), got '{}'",
                provider
            )));
        }

        let entry = keyring::Entry::new("commitbee", &provider_lower)
            .map_err(|e| Error::Keyring(e.to_string()))?;

        match entry.get_password() {
            Ok(_) => {
                eprintln!(
                    "{} API key for {} is stored in keychain",
                    style("✓").green().bold(),
                    provider_lower
                );
            }
            Err(keyring::Error::NoEntry) => {
                eprintln!(
                    "{} No API key found for {} in keychain",
                    style("✗").red().bold(),
                    provider_lower
                );
                eprintln!(
                    "  Store one with: {}",
                    style(format!("commitbee set-key {}", provider_lower)).yellow()
                );
            }
            Err(e) => {
                return Err(Error::Keyring(e.to_string()));
            }
        }

        Ok(())
    }

    // ─── Output Helpers ───

    fn print_status(&self, msg: &str) {
        eprintln!("{} {}", style("→").cyan(), msg);
    }

    fn print_info(&self, msg: &str) {
        eprintln!("{} {}", style("info:").cyan(), msg);
    }

    fn print_warning(&self, msg: &str) {
        eprintln!("{} {}", style("warning:").yellow().bold(), msg);
    }
}
