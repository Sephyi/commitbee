<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
-->

# Changelog

All notable changes to CommitBee are documented here.

## `v0.5.0` — Understand Everything (current)

- **Full signature extraction** — The LLM sees `pub fn connect(host: &str, timeout: Duration) -> Result<Connection>`, not just "Function connect." Extracted from tree-sitter AST nodes using a two-strategy approach: `child_by_field_name("body")` primary, body-node-kind scan fallback. Works across all 10 languages.
- **Signature diffs for modified symbols** — When a function signature changes, the prompt shows `[~] old_sig → new_sig` so the LLM understands exactly what was modified.
- **Cross-file connection detection** — Detects when a changed file calls a symbol defined in another changed file. Shown as `CONNECTIONS: validator calls parse() — both changed` in the prompt.
- **Semantic change classification** — Modified symbols are classified as whitespace-only or semantic via character-stream comparison. Formatting-only changes (cargo fmt, prettier) auto-detected as `style` type when all modified symbols are whitespace-only.
- **Dual old/new line tracking** — `classify_span_change` correctly tracks old-file and new-file line numbers independently, handling cases where symbols shift positions due to added/removed lines above them.
- **Token budget rebalance** — Symbol section gets 30% of budget (up from 20%) when signatures are present, since richer symbols reduce the LLM's dependency on raw diff.
- **BODY_NODE_KINDS coverage** — Signature extraction verified across all 10 languages with dedicated tests for Java, C, C++, Ruby, and C#.
- **Connection reliability** — Short symbol names (<4 chars) filtered to prevent false positives, short-circuit after 5 connections, sort+dedup for correctness.
- **Fixed false positive in breaking change detection** — Modified public symbols were incorrectly counted as "removed APIs", causing spurious `breaking_change` validator violations and retry exhaustion.

## `v0.4.0` — See Everything

- **10-language tree-sitter support** — Added Java, C, C++, Ruby, and C# to the existing Rust, TypeScript, JavaScript, Python, and Go. All languages are individually feature-gated and enabled by default. Disable any with `--no-default-features` + selective `--features lang-rust,lang-go,...`.
- **Custom prompt templates** — User-defined templates with `{{diff}}`, `{{symbols}}`, `{{files}}`, `{{type}}`, `{{scope}}` variables via `template_path` config.
- **Multi-language commit messages** — Generate messages in any language with `--locale` flag or `locale` config (e.g., `--locale de` for German).
- **Commit history style learning** — Learns from recent commit history to match your project's style (`learn_from_history`, `history_sample_size` config).
- **Rename detection** — Detects file renames with similarity percentage via `git diff --find-renames`, displayed as `old → new (N% similar)` in prompts and split suggestions. Configurable threshold (default 70%, set to 0 to disable).
- **Expanded secret scanning** — 25 built-in patterns across 13 categories (cloud providers, AI/ML, source control, communication, payment, database, cryptographic, generic). Pluggable engine: add custom regex patterns or disable built-ins by name via config.
- **Progress indicators** — Contextual `indicatif` spinners during pipeline phases (analyzing, scanning, generating). Auto-suppressed in non-TTY environments (git hooks, pipes).
- **Evaluation harness** — `cargo test --features eval` for structured LLM output quality benchmarking.
- **Fuzz testing** — `cargo-fuzz` targets for sanitizer and diff parser robustness.
- **Exclude files** — `--exclude <GLOB>` flag (repeatable) and `exclude_patterns` config option. Glob patterns filter files from analysis (e.g., `*.lock`, `**/*.generated.*`, `vendor/**`). CLI patterns additive with config.
- **Copy to clipboard** — `--clipboard` flag copies the generated message to the system clipboard and prints to stdout, skipping commit confirmation.

## `v0.3.1` — Trust, but Verify

- **Multi-pass corrective retry** — Validator checks LLM output against 7 rules and retries up to 3 times with targeted correction instructions
- **Subject length enforcement** — Rejects subjects exceeding 72-char first line with a clear error instead of silent truncation
- **Stronger prompt budget** — Character limit embedded directly in JSON template, "HARD LIMIT" phrasing for better small-model compliance
- **Default model: `qwen3.5:4b`** — Smaller (3.4GB), no thinking overhead, clean JSON output out of the box
- **Configurable thinking mode** — `think` config option for Ollama models that support reasoning separation

## `v0.3.0` — Read Between the Lines

- **Diff-shape fingerprinting + Jaccard clustering** — Splitter groups files by change shape and content vocabulary, not just directory
- **Evidence-based type inference** — Constraint rules from code analysis drive commit type selection (bug evidence → fix, mechanical → style, dependency-only → chore)
- **Robust LLM output parsing** — Sanitizer handles `<think>`/`<thought>` blocks, conversational preambles, noisy JSON extraction
- **Metadata-aware breaking change detection** — Detects MSRV bumps, engines.node, requires-python changes
- **Symbol tri-state tracking** — Added/removed/modified-signature differentiation in tree-sitter analysis
- **Primary change detection** — Identifies the single most significant change for subject anchoring
- **Post-generation validation** — Subject specificity validator ensures concrete entity naming
- **NUL-delimited git parsing** — Safe handling of paths with special characters
- **Parallel tree-sitter parsing** — rayon for CPU-bound parsing, tokio JoinSet for concurrent git fetching
- **Anti-hallucination prompt engineering** — EVIDENCE/CONSTRAINTS sections, negative examples, anti-copy rules

## `v0.2.0` — Commit, Don't Think

- **Cloud providers** — OpenAI-compatible and Anthropic streaming support
- **Commit splitting** — Automatic detection and splitting of multi-concern staged changes
- **Git hook integration** — `commitbee hook install/uninstall/status`
- **Shell completions** — bash, zsh, fish, powershell via `clap_complete`
- **Rich error diagnostics** — `miette` for actionable error messages
- **Multiple message generation** — `--generate N` with interactive candidate selection
- **Hierarchical config** — `figment`-based layering (CLI > Env > File > Defaults)
- **Structured logging** — `tracing` with `COMMITBEE_LOG` env filter
- **Doctor command** — `commitbee doctor` for connectivity and config checks
- **Secure key storage** — OS keychain via `keyring` (optional feature)
- **Body line wrapping** — Commit body text wrapped at 72 characters
