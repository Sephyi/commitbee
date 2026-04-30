// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::io::IsTerminal;
use std::path::PathBuf;

use console::style;
use dialoguer::{Confirm, Editor, Input, Select};
use globset::{Glob, GlobSetBuilder};
use tokio::signal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::cli::{Cli, Commands, HookAction};
use crate::config::Config;
use crate::domain::PromptContext;
use crate::domain::{ChangeStatus, CodeSymbol, CommitType, FileCategory, StagedChanges};
use crate::error::{Error, Result};
use crate::services::{
    analyzer::AnalyzerService,
    context::ContextBuilder,
    git::GitService,
    history::HistoryService,
    llm,
    progress::Progress,
    safety,
    sanitizer::{CommitSanitizer, CommitValidator},
    splitter::{CommitSplitter, SplitSuggestion},
    template,
};

pub struct App {
    cli: Cli,
    config: Config,
    cancel_token: CancellationToken,
}

impl App {
    /// Create a Progress instance that respects --porcelain (fully silent) and --verbose.
    fn make_progress(&self) -> Progress {
        if self.cli.porcelain {
            Progress::silent()
        } else {
            Progress::new(self.cli.verbose)
        }
    }
}

impl App {
    pub fn new(mut cli: Cli) -> Result<Self> {
        // --porcelain: machine-readable mode. Flag conflicts are rejected at
        // parse time via clap; subcommands need a runtime check because clap
        // cannot declaratively conflict a flag with all subcommands at once.
        //
        // Porcelain does NOT imply --yes: --yes commits for real, porcelain
        // only generates and prints. The two are mutually exclusive (enforced
        // via clap). --dry-run and --no-split are safe redundancies that just
        // make the short-circuit paths in generate_commit obvious.
        if cli.porcelain {
            if cli.command.is_some() {
                return Err(Error::Config(
                    "--porcelain cannot be combined with a subcommand".into(),
                ));
            }
            cli.dry_run = true;
            cli.no_split = true;
        }
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
        // Setup Ctrl+C handler with CancellationToken.
        // On registration failure, log the error and still call cancel.cancel()
        // so any running CancellationToken-aware task (e.g., LLM streams) gets
        // an explicit shutdown signal. This matches the legacy `.ok()` behavior
        // (which fell through to cancel on Err) while now surfacing the error
        // via tracing instead of silently discarding it (audit F-025).
        let cancel = self.cancel_token.clone();
        tokio::spawn(async move {
            if let Err(e) = signal::ctrl_c().await {
                warn!(error = %e, "failed to install Ctrl+C handler");
            }
            cancel.cancel();
        });

        // Handle subcommands
        if let Some(ref cmd) = self.cli.command {
            return self.handle_command(cmd).await;
        }

        self.generate_commit().await
    }

    #[tracing::instrument(
        skip_all,
        fields(provider = %self.config.provider, model = %self.config.model)
    )]
    async fn generate_commit(&mut self) -> Result<()> {
        if self.cancel_token.is_cancelled() {
            return Err(Error::Cancelled);
        }

        // Step 1: Discover repo and get changes
        let progress = self.make_progress();
        progress.phase("Analyzing staged changes...");

        let git = GitService::discover()?;
        let (changes, full_diff) = git
            .get_staged_changes(self.config.max_file_lines, self.config.rename_threshold)
            .await?;

        progress.info(&format!(
            "{} files with changes detected (+{} -{})",
            changes.files.len(),
            changes.stats.insertions,
            changes.stats.deletions
        ));

        // Step 1.5: Exclude files matching glob patterns
        let changes = self.apply_exclude_patterns(changes, &progress)?;

        // Step 2: Check for safety issues
        if safety::check_for_conflicts(&changes) {
            return Err(Error::MergeConflicts);
        }

        // Scan the full untruncated diff for secrets (not the per-file truncated diffs)
        let secret_patterns = safety::build_patterns(
            &self.config.custom_secret_patterns,
            &self.config.disabled_secret_patterns,
        );
        let secrets = safety::scan_full_diff_with_patterns(&full_diff, &secret_patterns);
        if !secrets.is_empty() {
            warn!(
                count = secrets.len(),
                "potential secrets detected in staged changes"
            );
            progress.warning("Potential secrets detected:");
            for s in &secrets {
                eprintln!(
                    "  {} in {} (line ~{})",
                    s.pattern_name,
                    s.file,
                    s.line.unwrap_or(0)
                );
            }

            // Cloud providers: always block when secrets detected
            if !self.cli.allow_secrets {
                return Err(Error::SecretsDetected {
                    patterns: secrets.iter().map(|s| s.pattern_name.clone()).collect(),
                });
            }

            // --allow-secrets passed: require interactive confirmation.
            // Skip the prompt when the user has opted out of interactivity
            // (either via --yes or --porcelain) — otherwise a silent blocking
            // prompt on piped stdin would hang the whole pipeline. Both
            // alternatives fall through to the non-interactive "fail closed"
            // branch below.
            if !self.cli.yes && !self.cli.porcelain && std::io::stdin().is_terminal() {
                progress.finish();
                eprintln!("\nwarning: Potential secrets detected in staged changes.");
                for s in &secrets {
                    eprintln!(
                        "  {} in {} (line ~{})",
                        s.pattern_name,
                        s.file,
                        s.line.unwrap_or(0)
                    );
                }
                eprintln!(
                    "Provider: {} ({})",
                    self.config.provider,
                    if self.config.provider == crate::config::Provider::Ollama {
                        &self.config.ollama_host
                    } else {
                        "cloud API"
                    }
                );
                eprint!("Send diff to LLM anyway? [y/N] ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
                if !input.trim().eq_ignore_ascii_case("y") {
                    return Err(Error::SecretsDetected {
                        patterns: secrets.iter().map(|s| s.pattern_name.clone()).collect(),
                    });
                }
            } else {
                // Non-interactive: always block even with --allow-secrets
                return Err(Error::SecretsDetected {
                    patterns: secrets.iter().map(|s| s.pattern_name.clone()).collect(),
                });
            }
        }

        if self.cancel_token.is_cancelled() {
            return Err(Error::Cancelled);
        }

        // Step 3: Pre-fetch file content and analyze with tree-sitter
        progress.phase("Extracting code symbols...");

        let analyzer = AnalyzerService::new()?;

        // Fetch all file content concurrently (async I/O via tokio JoinSet)
        let file_paths: Vec<PathBuf> = changes.files.iter().map(|f| f.path.clone()).collect();
        let (staged_map, head_map) = git.fetch_file_contents(&file_paths).await;

        // Parse symbols in parallel across CPU cores (rayon)
        let (symbols, symbol_diffs) =
            analyzer.extract_symbols(&changes.files, &staged_map, &head_map);

        debug!(count = symbols.len(), "symbols extracted");

        // Finish analysis spinner before any interactive prompts
        progress.finish();

        let is_interactive = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();

        // Step 3.5: Split detection
        if !self.cli.no_split && is_interactive && !self.cli.yes {
            let suggestion = CommitSplitter::analyze(&changes, &symbols);

            if let SplitSuggestion::SuggestSplit(groups) = suggestion {
                Self::display_split_suggestion(&groups, &changes);

                let split_confirm = Confirm::new()
                    .with_prompt("Split into separate commits?")
                    .default(true)
                    .interact()?;

                if split_confirm {
                    return self
                        .run_split_flow(&git, groups, &changes, &symbols, &symbol_diffs)
                        .await;
                }
                progress.info("Proceeding with single commit");
            }
        }

        // Step 3.7: History style learning (experimental)
        let history_prompt = if self.config.learn_from_history {
            debug!("learning commit style from history");
            match HistoryService::analyze(git.work_dir(), self.config.history_sample_size).await {
                Some(ctx) => {
                    let section = ctx.to_prompt_section(self.config.history_sample_size);
                    debug!(
                        conventional_ratio = ctx.conventional_ratio,
                        types = ctx.type_distribution.len(),
                        scopes = ctx.scope_patterns.len(),
                        "history analysis complete"
                    );
                    Some(section)
                }
                None => {
                    debug!("history analysis skipped (too few commits or git log failed)");
                    None
                }
            }
        } else {
            None
        };

        // Step 4: Build context
        let mut context = ContextBuilder::build(&changes, &symbols, &symbol_diffs, &self.config);
        context.history_context = history_prompt;
        debug!(prompt_chars = context.to_prompt().len(), "context built");

        let system_prompt = self.resolve_system_prompt()?;
        let prompt = self.resolve_user_prompt(&context)?;

        if self.cli.show_prompt {
            eprintln!("{}", style("--- PROMPT ---").dim());
            eprintln!("{}", prompt);
            eprintln!("{}", style("--- END PROMPT ---").dim());
            return Ok(());
        }

        if self.cancel_token.is_cancelled() {
            return Err(Error::Cancelled);
        }

        // Step 5: Generate commit message(s)
        let num_candidates = self.cli.generate;

        // Restart spinner for LLM generation phase
        let mut progress = self.make_progress();
        progress.phase(&format!(
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
                progress.phase(&format!(
                    "Generating candidate {}/{}...",
                    i + 1,
                    num_candidates
                ));
            } else {
                progress.phase("Generating...");
            }

            let (tx, mut rx) = mpsc::channel::<String>(64);

            // Only stream output for single generation (and never in porcelain mode — stderr must be silent)
            let show_stream = num_candidates == 1 && !self.cli.porcelain;
            let cancel_for_printer = self.cancel_token.clone();
            let spinner = progress.take_bar();

            let print_handle = tokio::spawn(async move {
                let mut first = true;
                loop {
                    tokio::select! {
                        _ = cancel_for_printer.cancelled() => break,
                        token = rx.recv() => {
                            match token {
                                Some(t) if show_stream => {
                                    if first {
                                        if let Some(ref bar) = spinner {
                                            bar.finish_and_clear();
                                        }
                                        first = false;
                                    }
                                    eprint!("{}", t);
                                }
                                Some(_) => {} // Suppress streaming for multi-gen
                                None => break,
                            }
                        }
                    }
                }
            });

            let raw_message = provider
                .generate(&prompt, &system_prompt, tx, self.cancel_token.clone())
                .await?;

            if let Err(e) = print_handle.await {
                warn!("print task panicked: {e}");
            }

            if num_candidates == 1 && !self.cli.porcelain {
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

            // Validate against evidence and retry once if violations found
            let raw_to_sanitize = self
                .validate_and_retry(&raw_message, &context, &provider, &prompt, &system_prompt)
                .await
                .unwrap_or(raw_message);

            match CommitSanitizer::sanitize(&raw_to_sanitize, &self.config.format) {
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
        let mut message = if candidates.len() == 1 {
            candidates.into_iter().next().unwrap()
        } else {
            self.select_candidate(&candidates)?
        };

        // Step 6.5: Interactive Review / Edit
        if !self.cli.yes && is_interactive && !self.cli.dry_run && !self.cli.clipboard {
            loop {
                eprintln!("\n{}", style("Commit message:").bold());
                eprintln!("{}", style(&message).green());
                eprintln!();

                let options = &["Commit", "Edit", "Refine", "Cancel"];
                let selection = Select::new()
                    .with_prompt("What would you like to do?")
                    .items(options)
                    .default(0)
                    .interact()
                    .map_err(|e| Error::Dialog(e.to_string()))?;

                match selection {
                    0 => break, // Commit
                    1 => {
                        if let Some(edited) = Editor::new()
                            .edit(&message)
                            .map_err(|e| Error::Dialog(e.to_string()))?
                            && !edited.trim().is_empty()
                        {
                            message = edited;
                        }
                    }
                    2 => {
                        let feedback: String = Input::new()
                            .with_prompt("What should be improved?")
                            .interact_text()
                            .map_err(|e| Error::Dialog(e.to_string()))?;

                        if !feedback.trim().is_empty() {
                            let progress = self.make_progress();
                            progress.phase("Refining message...");

                            match self
                                .refine_message(
                                    &provider,
                                    &prompt,
                                    &system_prompt,
                                    &message,
                                    &feedback,
                                    &context,
                                )
                                .await
                            {
                                Ok(refined) => {
                                    message = refined;
                                }
                                Err(e) => {
                                    warn!(error = %e, "failed to refine message");
                                    eprintln!(
                                        "{} Failed to refine message: {}",
                                        style("error:").red(),
                                        e
                                    );
                                }
                            }
                        }
                    }
                    _ => return Err(Error::Cancelled),
                }
            }
        }

        // Step 7: Clipboard / dry-run / commit
        if self.cli.clipboard {
            Self::copy_to_clipboard(message.clone()).await?;
            eprintln!("{} Copied to clipboard!", style("✓").green().bold());
            println!("{}", message);
            return Ok(());
        }

        if self.cli.dry_run {
            println!("{}", message);
            return Ok(());
        }

        // Auto-commit if --yes is set
        if self.cli.yes {
            git.commit(&message).await?;
            eprintln!("{} Committed!", style("✓").green().bold());
            return Ok(());
        }

        if !is_interactive {
            eprintln!("{}", style("warning:").yellow().bold());
            eprintln!("  Not a terminal. Use --yes to auto-confirm in scripts/hooks.");
            println!("{}", message);
            return Ok(());
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
                println!("Think: {}", self.config.think);
                println!("Rename threshold: {}%", self.config.rename_threshold);
                println!(
                    "Learn from history: {} (sample: {})",
                    self.config.learn_from_history, self.config.history_sample_size
                );
                if !self.config.exclude_patterns.is_empty() {
                    println!(
                        "Exclude patterns: {}",
                        self.config.exclude_patterns.join(", ")
                    );
                }
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
            Commands::Hook { action } => self.handle_hook(action).await,
            #[cfg(feature = "secure-storage")]
            Commands::SetKey { provider } => self.set_api_key(provider),
            #[cfg(feature = "secure-storage")]
            Commands::GetKey { provider } => self.get_api_key(provider),
            #[cfg(feature = "eval")]
            Commands::Eval {
                fixtures_dir,
                filter,
            } => {
                let runner = crate::eval::EvalRunner::new(fixtures_dir.clone(), filter.clone());
                runner.run().await
            }
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
                eprint!("  {}: ", other);
                match self.config.api_key.as_ref() {
                    None => {
                        eprintln!("{} (no API key configured)", style("MISSING").red().bold());
                    }
                    Some(_key) => {
                        // Build the provider to reach verify(). Construction itself
                        // can fail (e.g. HTTP client build); keep that non-fatal so
                        // the rest of `doctor` still runs. Do not echo any portion
                        // of the API key (even a partial suffix is correlatable
                        // across logs/screenshots).
                        match llm::create_provider(&self.config) {
                            Err(e) => {
                                eprintln!("{}: {}", style("ERROR").red().bold(), e);
                            }
                            Ok(provider) => match provider.verify().await {
                                Ok(()) => {
                                    eprintln!("{}", style("reachable").green().bold());
                                }
                                Err(e) => {
                                    eprintln!("{}: {}", style("ERROR").red().bold(), e);
                                }
                            },
                        }
                    }
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
        symbol_diffs: &[crate::domain::diff::SymbolDiff],
    ) -> Result<()> {
        // Safety: check for files with both staged and unstaged changes
        let overlap = git.has_unstaged_overlap().await?;
        if !overlap.is_empty() {
            eprintln!(
                "{} Cannot split: some staged files also have unstaged changes:",
                style("warning:").yellow().bold()
            );
            for path in &overlap {
                eprintln!("  {}", path.display());
            }
            eprintln!(
                "{} Stash or commit unstaged changes first, or use --no-split",
                style("info:").cyan()
            );
            return Err(Error::SplitAborted);
        }

        let progress = self.make_progress();
        // Generate messages for each group
        progress.phase(&format!(
            "Contacting {} ({})...",
            self.config.provider, self.config.model
        ));
        progress.finish();

        let provider = llm::create_provider(&self.config)?;
        provider.verify().await?;

        let system_prompt = self.resolve_system_prompt()?;
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

            let sub_diffs: Vec<_> = symbol_diffs
                .iter()
                .filter(|d| sub_changes.files.iter().any(|f| f.path == d.file))
                .cloned()
                .collect();
            let mut context =
                ContextBuilder::build(&sub_changes, &sub_symbols, &sub_diffs, &self.config);
            context.group_rationale = Some(Self::infer_group_rationale(
                &sub_changes,
                &group.commit_type,
            ));
            let prompt = self.resolve_user_prompt(&context)?;

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
                .generate(&prompt, &system_prompt, tx, self.cancel_token.clone())
                .await?;

            if let Err(e) = print_handle.await {
                warn!("print task panicked: {e}");
            }

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

            let raw_to_sanitize = self
                .validate_and_retry(&raw_message, &context, &provider, &prompt, &system_prompt)
                .await
                .unwrap_or(raw_message);

            let message = CommitSanitizer::sanitize(&raw_to_sanitize, &self.config.format)?;
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

        // Execute: unstage all, then stage+commit per group.
        // NOTE: This is non-atomic — if an intermediate commit fails, earlier
        // commits are already applied with no automatic rollback. The index
        // state between unstage_all() and stage_files() is also a TOCTOU window.
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

    /// Generate a short rationale describing why files were grouped together.
    fn infer_group_rationale(changes: &StagedChanges, commit_type: &CommitType) -> String {
        let file_count = changes.files.len();
        let categories: Vec<_> = changes.files.iter().map(|f| f.category).collect();

        // All same category?
        if categories.iter().all(|c| *c == categories[0]) {
            let cat = match categories[0] {
                FileCategory::Docs => "documentation",
                FileCategory::Test => "test",
                FileCategory::Config => "configuration",
                FileCategory::Build => "build/CI",
                FileCategory::Source => "source",
                FileCategory::Other => "miscellaneous",
            };
            return format!(
                "{} {} changes across {} files",
                commit_type.as_str(),
                cat,
                file_count
            );
        }

        // Mixed categories
        let source_count = categories
            .iter()
            .filter(|c| **c == FileCategory::Source)
            .count();
        let test_count = categories
            .iter()
            .filter(|c| **c == FileCategory::Test)
            .count();

        if source_count > 0 && test_count > 0 {
            format!(
                "{} changes in {} source + {} test files",
                commit_type.as_str(),
                source_count,
                test_count
            )
        } else {
            format!(
                "{} changes across {} files",
                commit_type.as_str(),
                file_count
            )
        }
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
                        ChangeStatus::Renamed => "[R]",
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

    async fn handle_hook(&self, action: &HookAction) -> Result<()> {
        match action {
            HookAction::Install => self.hook_install().await,
            HookAction::Uninstall => self.hook_uninstall().await,
            HookAction::Status => self.hook_status().await,
        }
    }

    async fn hook_dir(&self) -> Result<PathBuf> {
        // Verify we're in a git repo first
        let _git = GitService::discover()?;

        // Use `tokio::process::Command` so the spawn does not block a Tokio
        // worker thread — `hook_dir` is reached via `app.run().await`, so it
        // is always invoked under the runtime.
        let output: std::process::Output = tokio::process::Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .output()
            .await?;

        if !output.status.success() {
            return Err(Error::Git("Cannot find .git directory".into()));
        }

        let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PathBuf::from(git_dir).join("hooks"))
    }

    async fn hook_path(&self) -> Result<PathBuf> {
        Ok(self.hook_dir().await?.join("prepare-commit-msg"))
    }

    async fn hook_install(&self) -> Result<()> {
        let hooks_dir = self.hook_dir().await?;
        let hook_path = hooks_dir.join("prepare-commit-msg");
        let backup_path = hooks_dir.join("prepare-commit-msg.commitbee-backup");

        // Create hooks directory if needed
        std::fs::create_dir_all(&hooks_dir)?;

        // Back up existing hook if present and not ours
        if hook_path.exists() {
            let content = match std::fs::read_to_string(&hook_path) {
                Ok(c) => c,
                Err(e) => {
                    warn!(path = %hook_path.display(), error = %e, "failed to read existing hook");
                    String::new()
                }
            };
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
    printf '%s\n' "$MSG" > "$COMMIT_MSG_FILE"
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

    async fn hook_uninstall(&self) -> Result<()> {
        let hooks_dir = self.hook_dir().await?;
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
        let content = match std::fs::read_to_string(&hook_path) {
            Ok(c) => c,
            Err(e) => {
                warn!(path = %hook_path.display(), error = %e, "failed to read hook file");
                String::new()
            }
        };
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

    async fn hook_status(&self) -> Result<()> {
        let hook_path = self.hook_path().await?;

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

        let content = match std::fs::read_to_string(&hook_path) {
            Ok(c) => c,
            Err(e) => {
                warn!(path = %hook_path.display(), error = %e, "failed to read hook file");
                String::new()
            }
        };
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

    // ─── Post-Generation Validation ───

    /// Validate raw LLM output against evidence flags. If violations exist,
    /// re-prompt with corrections appended, up to 3 attempts. Returns the
    /// corrected raw output, or None if no retry was needed.
    async fn validate_and_retry(
        &self,
        raw: &str,
        context: &PromptContext,
        provider: &llm::LlmBackend,
        original_prompt: &str,
        system_prompt: &str,
    ) -> Option<String> {
        const MAX_RETRIES: usize = 3;

        let mut current_raw = raw.to_string();

        for attempt in 1..=MAX_RETRIES {
            let structured = match CommitSanitizer::parse_structured(&current_raw) {
                Some(s) => s,
                None => {
                    if attempt == 1 {
                        return None; // Can't parse, let sanitize() handle it
                    }
                    break; // Retry produced unparseable output, use last good one
                }
            };

            let violations = CommitValidator::validate(
                &structured,
                context.has_bug_evidence,
                context.is_mechanical,
                context.public_api_removed_count,
                context.is_dependency_only,
            );

            if violations.is_empty() {
                return if attempt == 1 {
                    None // First pass clean, use original
                } else {
                    debug!(attempt, "retry resolved all violations");
                    Some(current_raw)
                };
            }

            for v in &violations {
                debug!(attempt, violation = %v, "validation failed");
            }
            debug!(
                attempt,
                violations = violations.len(),
                "retrying with corrections"
            );

            let corrections = CommitValidator::format_corrections(&violations);
            let retry_prompt = format!("{}\n{}", original_prompt, corrections);

            let (tx, mut rx) = mpsc::channel::<String>(64);
            let cancel = self.cancel_token.clone();
            let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

            match provider
                .generate(&retry_prompt, system_prompt, tx, cancel)
                .await
            {
                Ok(retry_raw) if !retry_raw.trim().is_empty() => {
                    let _ = drain_handle.await;
                    current_raw = retry_raw;
                }
                _ => {
                    let _ = drain_handle.await;
                    warn!(attempt, "retry failed or empty, using previous output");
                    break;
                }
            }
        }

        // Exhausted retries — return best effort if we did any retries
        if current_raw != raw {
            warn!("max retries exhausted, using best-effort output");
            Some(current_raw)
        } else {
            None
        }
    }

    // ─── Prompt Helpers ───

    /// Resolve the system prompt: load from file if configured, otherwise use built-in.
    fn resolve_system_prompt(&self) -> Result<String> {
        if let Some(ref path) = self.config.system_prompt_path {
            template::load_file(path)
        } else {
            Ok(llm::SYSTEM_PROMPT.to_string())
        }
    }

    /// Resolve the user prompt: render from template if configured, otherwise use default.
    fn resolve_user_prompt(&self, context: &PromptContext) -> Result<String> {
        if let Some(ref path) = self.config.template_path {
            let symbols_text = self.build_symbols_text(context);
            let scope_text = context.suggested_scope.as_deref().unwrap_or("");

            let mut vars = std::collections::HashMap::new();
            vars.insert("diff", context.truncated_diff.as_str());
            vars.insert("symbols", symbols_text.as_str());
            vars.insert("files", context.file_breakdown.as_str());
            vars.insert("type", context.suggested_type.as_str());
            vars.insert("scope", scope_text);
            vars.insert("evidence", context.change_summary.as_str());
            vars.insert("constraints", context.public_api_removed.as_str());

            template::render_template(path, &vars)
        } else {
            Ok(context.to_prompt())
        }
    }

    /// Build combined symbol text from all symbol sections in a PromptContext.
    fn build_symbols_text(&self, context: &PromptContext) -> String {
        let mut parts = Vec::new();
        if !context.symbols_added.is_empty() {
            parts.push(format!(
                "Added:\n  {}",
                context.symbols_added.replace('\n', "\n  ")
            ));
        }
        if !context.symbols_removed.is_empty() {
            parts.push(format!(
                "Removed:\n  {}",
                context.symbols_removed.replace('\n', "\n  ")
            ));
        }
        if !context.symbols_modified.is_empty() {
            parts.push(format!(
                "Modified:\n  {}",
                context.symbols_modified.replace('\n', "\n  ")
            ));
        }
        parts.join("\n")
    }

    /// Refine a commit message based on user feedback.
    async fn refine_message(
        &self,
        provider: &llm::LlmBackend,
        original_prompt: &str,
        system_prompt: &str,
        current_message: &str,
        feedback: &str,
        context: &PromptContext,
    ) -> Result<String> {
        let refinement_prompt =
            self.resolve_refinement_prompt(original_prompt, current_message, feedback)?;

        let (tx, mut rx) = mpsc::channel::<String>(64);
        let cancel = self.cancel_token.clone();

        // Print streaming refinement
        let print_handle = tokio::spawn(async move {
            while let Some(t) = rx.recv().await {
                eprint!("{}", t);
            }
        });

        let raw_refined = provider
            .generate(&refinement_prompt, system_prompt, tx, cancel)
            .await?;

        print_handle.await.ok();
        eprintln!();

        // Validate and sanitize
        let refined_to_sanitize = self
            .validate_and_retry(
                &raw_refined,
                context,
                provider,
                &refinement_prompt,
                system_prompt,
            )
            .await
            .unwrap_or(raw_refined);

        CommitSanitizer::sanitize(&refined_to_sanitize, &self.config.format)
    }

    /// Resolve the refinement prompt.
    fn resolve_refinement_prompt(
        &self,
        original_prompt: &str,
        previous_message: &str,
        feedback: &str,
    ) -> Result<String> {
        Ok(format!(
            "{}\n\n---\nRefine the commit message based on the following user feedback:\n\"{}\"\n\nPrevious candidate was:\n\"{}\"\n\nRespond with ONLY the refined JSON object.",
            original_prompt, feedback, previous_message
        ))
    }

    // ─── Exclude Helpers ───

    /// Filter staged changes by removing files matching exclude glob patterns.
    /// Returns the filtered changes. Excluded files are listed in output.
    fn apply_exclude_patterns(
        &self,
        mut changes: StagedChanges,
        progress: &Progress,
    ) -> Result<StagedChanges> {
        if self.config.exclude_patterns.is_empty() {
            return Ok(changes);
        }

        let mut builder = GlobSetBuilder::new();
        for pattern in &self.config.exclude_patterns {
            let glob = Glob::new(pattern).map_err(|e| {
                Error::Config(format!("Invalid exclude pattern '{}': {}", pattern, e))
            })?;
            builder.add(glob);
        }
        let glob_set = builder
            .build()
            .map_err(|e| Error::Config(format!("Failed to build exclude patterns: {}", e)))?;

        let original_count = changes.files.len();
        let mut excluded: Vec<PathBuf> = Vec::new();

        changes.files.retain(|f| {
            if glob_set.is_match(&f.path) {
                excluded.push(f.path.clone());
                false
            } else {
                true
            }
        });

        if !excluded.is_empty() {
            // Recalculate stats from remaining files
            changes.stats.files_changed = changes.files.len();
            changes.stats.insertions = changes.files.iter().map(|f| f.additions).sum();
            changes.stats.deletions = changes.files.iter().map(|f| f.deletions).sum();

            progress.info(&format!(
                "Excluded {}/{} files matching patterns:",
                excluded.len(),
                original_count,
            ));
            for path in &excluded {
                debug!(path = %path.display(), "excluded by pattern");
            }
        }

        if changes.files.is_empty() {
            return Err(Error::NoStagedChanges);
        }

        Ok(changes)
    }

    // ─── Clipboard Helpers ───

    /// Copy text to the system clipboard using the arboard crate.
    ///
    /// `arboard` is a synchronous library and may briefly block on the X11 /
    /// Wayland / Cocoa clipboard daemon, so we delegate to
    /// `tokio::task::spawn_blocking` to avoid stalling a runtime worker.
    async fn copy_to_clipboard(text: String) -> Result<()> {
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut clipboard = arboard::Clipboard::new().map_err(|e| {
                Error::Config(format!(
                    "Failed to initialize clipboard: {e}. If on Linux, ensure x11 or wayland dependencies are installed."
                ))
            })?;

            clipboard
                .set_text(text)
                .map_err(|e| Error::Config(format!("Failed to copy to clipboard: {e}")))?;

            Ok(())
        })
        .await
        .map_err(|e| Error::Config(format!("clipboard task failed to join: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_refinement_prompt() {
        let app = App {
            cli: Cli::default(),
            config: Config::default(),
            cancel_token: CancellationToken::new(),
        };

        let original_prompt = "Original prompt";
        let previous_message = "Previous message";
        let feedback = "Make it shorter";

        let result = app
            .resolve_refinement_prompt(original_prompt, previous_message, feedback)
            .unwrap();

        assert!(result.contains("Original prompt"));
        assert!(result.contains("Previous message"));
        assert!(result.contains("Make it shorter"));
        assert!(result.contains("Respond with ONLY the refined JSON object."));
    }
}
