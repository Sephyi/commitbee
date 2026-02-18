// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;

use console::style;
use dialoguer::Confirm;
use tokio::signal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::cli::{Cli, Commands, HookAction};
use crate::config::Config;
use crate::domain::{ChangeStatus, CodeSymbol, StagedChanges};
use crate::error::{Error, Result};
use crate::services::{
    analyzer::AnalyzerService,
    context::ContextBuilder,
    git::GitService,
    llm, safety,
    sanitizer::CommitSanitizer,
    splitter::{CommitSplitter, SplitSuggestion},
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

        // Step 3.5: Split detection
        if !self.cli.no_split {
            let is_interactive = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();

            if is_interactive && !self.cli.yes {
                let suggestion = CommitSplitter::analyze(&changes, &symbols);

                if let SplitSuggestion::SuggestSplit(groups) = suggestion {
                    Self::display_split_suggestion(&groups, &changes);

                    let split_confirm = Confirm::new()
                        .with_prompt("Split into separate commits?")
                        .default(true)
                        .interact()?;

                    if split_confirm {
                        return self.run_split_flow(&git, groups, &changes, &symbols).await;
                    }
                    self.print_info("Proceeding with single commit");
                }
            }
        }

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

        // Step 5: Generate commit message(s)
        let num_candidates = self.cli.generate;

        self.print_status(&format!(
            "Contacting {} ({})...",
            self.config.provider, self.config.model
        ));

        let provider = llm::create_provider(&self.config)?;
        debug!(provider = provider.name(), "verifying provider");
        provider.verify().await?;

        let mut candidates: Vec<String> = Vec::new();

        for i in 0..num_candidates {
            if self.cancel_token.is_cancelled() {
                return Err(Error::Cancelled);
            }

            if num_candidates > 1 {
                eprintln!(
                    "{} Generating candidate {}/{}...",
                    style("info:").cyan(),
                    i + 1,
                    num_candidates
                );
            } else {
                eprintln!("{} Generating...\n", style("info:").cyan());
            }

            let (tx, mut rx) = mpsc::channel::<String>(64);

            // Only stream output for single generation
            let show_stream = num_candidates == 1;
            let cancel_for_printer = self.cancel_token.clone();
            let print_handle = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancel_for_printer.cancelled() => break,
                        token = rx.recv() => {
                            match token {
                                Some(t) if show_stream => eprint!("{}", t),
                                Some(_) => {} // Suppress streaming for multi-gen
                                None => break,
                            }
                        }
                    }
                }
            });

            let raw_message = provider
                .generate(&prompt, tx, self.cancel_token.clone())
                .await?;

            let _ = print_handle.await;

            if num_candidates == 1 {
                eprintln!(); // Newline after streaming
            }

            if raw_message.trim().is_empty() {
                warn!(candidate = i + 1, "empty response from LLM, skipping");
                continue;
            }

            debug!(
                raw_len = raw_message.len(),
                candidate = i + 1,
                "sanitizing LLM response"
            );
            match CommitSanitizer::sanitize(&raw_message, &self.config.format) {
                Ok(msg) => candidates.push(msg),
                Err(e) => {
                    warn!(candidate = i + 1, error = %e, "failed to sanitize candidate");
                }
            }
        }

        if candidates.is_empty() {
            return Err(Error::Provider {
                provider: provider.name().into(),
                message: "No valid commit messages generated".into(),
            });
        }

        // Step 6: Select message
        let message = if candidates.len() == 1 {
            candidates.into_iter().next().unwrap()
        } else {
            self.select_candidate(&candidates)?
        };

        // Step 7: Confirm and commit
        if self.cli.dry_run {
            println!("\n{}", message);
            return Ok(());
        }

        let is_interactive = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();

        if !self.cli.yes {
            if !is_interactive {
                eprintln!("{}", style("warning:").yellow().bold());
                eprintln!("  Not a terminal. Use --yes to auto-confirm in scripts/hooks.");
                println!("\n{}", message);
                return Ok(());
            }

            // For single candidate (already shown via streaming), just confirm
            if num_candidates == 1 {
                eprintln!("\n{}", style("Generated commit message:").bold());
                eprintln!("{}", style(&message).green());
                eprintln!();
            }

            let confirm = Confirm::new()
                .with_prompt("Create commit with this message?")
                .default(true)
                .interact()?;

            if !confirm {
                return Err(Error::Cancelled);
            }
        }

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
            Commands::Hook { action } => self.handle_hook(action),
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

    // ─── Split Detection ───

    async fn run_split_flow(
        &self,
        git: &GitService,
        groups: Vec<crate::services::splitter::CommitGroup>,
        changes: &StagedChanges,
        symbols: &[CodeSymbol],
    ) -> Result<()> {
        // Safety: check for files with both staged and unstaged changes
        let overlap = git.has_unstaged_overlap().await?;
        if !overlap.is_empty() {
            self.print_warning("Cannot split: some staged files also have unstaged changes:");
            for path in &overlap {
                eprintln!("  {}", path.display());
            }
            self.print_info("Stash or commit unstaged changes first, or use --no-split");
            return Err(Error::SplitAborted);
        }

        // Generate messages for each group
        self.print_status(&format!(
            "Contacting {} ({})...",
            self.config.provider, self.config.model
        ));

        let provider = llm::create_provider(&self.config)?;
        provider.verify().await?;

        let mut commit_messages: Vec<(String, Vec<PathBuf>)> = Vec::new();

        for (i, group) in groups.iter().enumerate() {
            if self.cancel_token.is_cancelled() {
                return Err(Error::Cancelled);
            }

            eprintln!(
                "{} Generating message for group {}/{}...",
                style("info:").cyan(),
                i + 1,
                groups.len(),
            );

            // Build sub-context for this group
            let sub_changes = changes.subset(&group.files);
            let sub_symbols: Vec<CodeSymbol> = symbols
                .iter()
                .filter(|s| group.files.contains(&s.file))
                .cloned()
                .collect();

            let context = ContextBuilder::build(&sub_changes, &sub_symbols, &self.config);
            let prompt = context.to_prompt();

            if self.cli.show_prompt {
                eprintln!(
                    "{}",
                    style(format!("--- PROMPT (Group {}) ---", i + 1)).dim()
                );
                eprintln!("{}", prompt);
                eprintln!("{}", style("--- END PROMPT ---").dim());
            }

            let (tx, mut rx) = mpsc::channel::<String>(64);
            let cancel_for_printer = self.cancel_token.clone();
            let print_handle = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancel_for_printer.cancelled() => break,
                        token = rx.recv() => {
                            match token {
                                Some(_) => {}
                                None => break,
                            }
                        }
                    }
                }
            });

            let raw_message = provider
                .generate(&prompt, tx, self.cancel_token.clone())
                .await?;

            let _ = print_handle.await;

            if raw_message.trim().is_empty() {
                return Err(Error::Provider {
                    provider: provider.name().into(),
                    message: format!("Empty response for group {}", i + 1),
                });
            }

            debug!(
                raw_len = raw_message.len(),
                group = i + 1,
                "sanitizing split group response"
            );
            let message = CommitSanitizer::sanitize(&raw_message, &self.config.format)?;
            commit_messages.push((message, group.files.clone()));
        }

        // Display overview
        Self::display_split_overview(&commit_messages);

        // Dry run: stop here
        if self.cli.dry_run {
            for (msg, _) in &commit_messages {
                println!("\n{}", msg);
            }
            return Ok(());
        }

        // Confirm
        let confirm = Confirm::new()
            .with_prompt(format!("Create {} commits?", commit_messages.len()))
            .default(true)
            .interact()?;

        if !confirm {
            return Err(Error::Cancelled);
        }

        // Execute: unstage all, then stage+commit per group
        for (i, (message, files)) in commit_messages.iter().enumerate() {
            git.unstage_all().await?;
            git.stage_files(files).await?;
            git.commit(message).await?;

            eprintln!(
                "{} Commit {}/{}: {}",
                style("✓").green().bold(),
                i + 1,
                commit_messages.len(),
                message.lines().next().unwrap_or(""),
            );
        }

        eprintln!(
            "\n{} {} commits created!",
            style("✓").green().bold(),
            commit_messages.len(),
        );

        Ok(())
    }

    fn display_split_suggestion(
        groups: &[crate::services::splitter::CommitGroup],
        changes: &StagedChanges,
    ) {
        eprintln!();
        eprintln!(
            "{} Commit split suggested — {} logical change groups detected:",
            style("⚡").yellow(),
            groups.len(),
        );
        eprintln!();

        for (i, group) in groups.iter().enumerate() {
            let scope_str = group
                .scope
                .as_ref()
                .map(|s| format!("({})", s))
                .unwrap_or_default();
            let file_count = group.files.len();
            let files_label = if file_count == 1 { "file" } else { "files" };

            eprintln!(
                "  Group {}: {}{}  [{} {}]",
                i + 1,
                group.commit_type.as_str(),
                scope_str,
                file_count,
                files_label,
            );

            for file_path in &group.files {
                if let Some(fc) = changes.files.iter().find(|f| f.path == *file_path) {
                    let status = match fc.status {
                        ChangeStatus::Added => "[+]",
                        ChangeStatus::Modified => "[M]",
                        ChangeStatus::Deleted => "[-]",
                    };
                    eprintln!(
                        "    {} {} (+{} -{})",
                        status,
                        file_path.display(),
                        fc.additions,
                        fc.deletions,
                    );
                }
            }
            eprintln!();
        }
    }

    fn display_split_overview(commits: &[(String, Vec<PathBuf>)]) {
        eprintln!();
        eprintln!("{}", style("→ Proposed commits:").cyan().bold());
        eprintln!();

        for (i, (message, files)) in commits.iter().enumerate() {
            let first_line = message.lines().next().unwrap_or("(empty)");
            eprintln!(
                "  Commit {}/{}: {}",
                i + 1,
                commits.len(),
                style(first_line).green(),
            );

            let files_str: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
            eprintln!("    Files: {}", files_str.join(", "));
            eprintln!();
        }
    }

    // ─── Candidate Selection ───

    fn select_candidate(&self, candidates: &[String]) -> Result<String> {
        if self.cli.yes {
            return Ok(candidates[0].clone());
        }

        let is_interactive = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();

        if !is_interactive || self.cli.dry_run {
            // Non-interactive: print all candidates
            for (i, msg) in candidates.iter().enumerate() {
                eprintln!("\n{}", style(format!("--- Candidate {} ---", i + 1)).dim());
                println!("{}", msg);
            }
            return Ok(candidates[0].clone());
        }

        // Interactive: show summary of each and let user pick
        eprintln!();
        let items: Vec<String> = candidates
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                let first_line = msg.lines().next().unwrap_or("(empty)");
                format!("[{}] {}", i + 1, first_line)
            })
            .collect();

        let selection = dialoguer::Select::new()
            .with_prompt("Pick a commit message")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| Error::Dialog(e.to_string()))?;

        let chosen = &candidates[selection];
        eprintln!("\n{}", style("Selected:").bold());
        eprintln!("{}", style(chosen).green());
        eprintln!();

        Ok(chosen.clone())
    }

    // ─── Hook Commands ───

    fn handle_hook(&self, action: &HookAction) -> Result<()> {
        match action {
            HookAction::Install => self.hook_install(),
            HookAction::Uninstall => self.hook_uninstall(),
            HookAction::Status => self.hook_status(),
        }
    }

    fn hook_dir(&self) -> Result<PathBuf> {
        // Verify we're in a git repo first
        let _git = GitService::discover()?;

        let output = std::process::Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .output()?;

        if !output.status.success() {
            return Err(Error::Git("Cannot find .git directory".into()));
        }

        let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PathBuf::from(git_dir).join("hooks"))
    }

    fn hook_path(&self) -> Result<PathBuf> {
        Ok(self.hook_dir()?.join("prepare-commit-msg"))
    }

    fn hook_install(&self) -> Result<()> {
        let hooks_dir = self.hook_dir()?;
        let hook_path = hooks_dir.join("prepare-commit-msg");
        let backup_path = hooks_dir.join("prepare-commit-msg.commitbee-backup");

        // Create hooks directory if needed
        std::fs::create_dir_all(&hooks_dir)?;

        // Back up existing hook if present and not ours
        if hook_path.exists() {
            let content = std::fs::read_to_string(&hook_path).unwrap_or_default();
            if content.contains("# commitbee hook") {
                eprintln!(
                    "{} Hook already installed at {}",
                    style("✓").green().bold(),
                    hook_path.display()
                );
                return Ok(());
            }
            std::fs::copy(&hook_path, &backup_path)?;
            eprintln!(
                "{} Backed up existing hook to {}",
                style("info:").cyan(),
                backup_path.display()
            );
        }

        let hook_script = r#"#!/bin/sh
# commitbee hook — auto-generated, do not edit
# Generates commit messages using commitbee when committing interactively.
# Skips merge, squash, amend, and message-provided commits.

COMMIT_MSG_FILE="$1"
COMMIT_SOURCE="$2"

# Skip non-interactive commits (merge, squash, message, amend)
case "$COMMIT_SOURCE" in
    merge|squash|message|commit)
        exit 0
        ;;
esac

# Only run if commitbee is available
if ! command -v commitbee >/dev/null 2>&1; then
    exit 0
fi

# Generate commit message and write to file
MSG=$(commitbee --yes --dry-run 2>/dev/null)
if [ $? -eq 0 ] && [ -n "$MSG" ]; then
    echo "$MSG" > "$COMMIT_MSG_FILE"
fi
"#;

        // Write to temp file first, then rename (atomic)
        let temp_path = hooks_dir.join(".prepare-commit-msg.tmp");
        std::fs::write(&temp_path, hook_script)?;

        // Set executable permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&temp_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&temp_path, perms)?;
        }

        std::fs::rename(&temp_path, &hook_path)?;

        eprintln!(
            "{} Hook installed at {}",
            style("✓").green().bold(),
            hook_path.display()
        );
        Ok(())
    }

    fn hook_uninstall(&self) -> Result<()> {
        let hooks_dir = self.hook_dir()?;
        let hook_path = hooks_dir.join("prepare-commit-msg");
        let backup_path = hooks_dir.join("prepare-commit-msg.commitbee-backup");

        if !hook_path.exists() {
            eprintln!(
                "{} No hook found at {}",
                style("info:").cyan(),
                hook_path.display()
            );
            return Ok(());
        }

        // Verify it's our hook before removing
        let content = std::fs::read_to_string(&hook_path).unwrap_or_default();
        if !content.contains("# commitbee hook") {
            return Err(Error::Git(format!(
                "Hook at {} was not installed by commitbee. Remove manually if intended.",
                hook_path.display()
            )));
        }

        std::fs::remove_file(&hook_path)?;

        // Restore backup if exists
        if backup_path.exists() {
            std::fs::rename(&backup_path, &hook_path)?;
            eprintln!(
                "{} Restored previous hook from backup",
                style("info:").cyan()
            );
        }

        eprintln!(
            "{} Hook removed from {}",
            style("✓").green().bold(),
            hook_path.display()
        );
        Ok(())
    }

    fn hook_status(&self) -> Result<()> {
        let hook_path = self.hook_path()?;

        if !hook_path.exists() {
            eprintln!(
                "{} No prepare-commit-msg hook installed",
                style("✗").red().bold()
            );
            eprintln!(
                "  Install with: {}",
                style("commitbee hook install").yellow()
            );
            return Ok(());
        }

        let content = std::fs::read_to_string(&hook_path).unwrap_or_default();
        if content.contains("# commitbee hook") {
            eprintln!(
                "{} CommitBee hook is installed at {}",
                style("✓").green().bold(),
                hook_path.display()
            );
        } else {
            eprintln!(
                "{} A prepare-commit-msg hook exists but was not installed by commitbee",
                style("info:").cyan()
            );
        }

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
