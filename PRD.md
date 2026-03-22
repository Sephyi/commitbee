<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
-->

# CommitBee — Product Requirements Document

**Version**: 4.2
**Date**: 2026-03-22
**Status**: Active  
**Author**: [Sephyi](https://github.com/Sephyi) + [Claude Opus 4.6](https://www.anthropic.com/news/claude-opus-4-6)  

## Changelog

<details>
<summary>Revision history (v3.3 → v4.2)</summary>

| Version | Date       | Summary |
|---------|------------|---------|
| 4.2     | 2026-03-22 | v0.5.0 hardening: security fixes (SSRF prevention, streaming caps), prompt optimization (budget fix, evidence omission, emoji removal), eval harness (36 fixtures, per-type reporting), test coverage (15+ new tests), API hygiene (pub(crate) demotions), 5 fuzz targets. 367 tests. |
| 4.1     | 2026-03-22 | AST context overhaul (v0.5.0): full signature extraction from tree-sitter nodes, semantic change classification (whitespace vs body vs signature), old→new signature diffs, cross-file connection detection, formatting auto-detection via symbols. 367 tests. |
| 4.0     | 2026-03-13 | PRD normalization: aligned phases with shipped versions (v0.2.0/v0.3.x/v0.4.0), collapsed revision history, unified status markers, resolved stale critical issues, canonicalized test count to 308, removed dead cross-references. FR-031 (Exclude Files) and FR-033 (Copy to Clipboard) shipped. |
| 3.3     | 2026-03-13 | v0.4.0 full feature completion — FR-030 (Custom Prompt Templates), FR-032 (Multi-Language), FR-036 (Tree-sitter Query Patterns), FR-057 (Additional Languages), FR-058 (History Learning), TR-006 (Eval Harness), TR-007 (Fuzzing). 308 tests. |
| 3.2     | 2026-03-13 | FR-035 (Rename Detection), FR-037 (Expanded Secret Scanning), FR-038 (Progress Indicators). 202 tests. |
| 3.1     | 2026-03-13 | Deep codebase audit + streaming hardening: `Provider::new()` returns `Result`, 1 MB response cap, EOF buffer parsing, zero-allocation streaming, HTTP error body propagation. 188 tests. |
| 3.0     | 2026-03-08 | v0.3.1 multi-pass retry + prompt enforcement. FR-041 expanded to 7 rules. 182 tests. |
| 2.9     | 2026-03-08 | v0.3.1 patch: default model → `qwen3.5:4b`, subject length enforcement, `think` config option. |
| 2.8     | 2026-03-08 | v0.3.0 release prep — sanitizer robustness, splitter Jaccard clustering, simplified prompts, NUL-delimited git parsing. 178 tests. |
| 2.7     | 2026-03-08 | Splitter precision + subject quality + metadata breaking detection. 169 tests. |
| 2.6     | 2026-03-08 | Message quality overhaul — FR-041, FR-034 partial, FR-023 enhanced, PE-001/PE-002 updates. 168 tests. |
| 2.5     | 2026-02-22 | PRD structural cleanup (FR placement fixes). |
| 2.4     | 2026-02-22 | Conventional Commits 1.0.0 spec compliance, symbol dedup. 133 tests. |
| 2.3     | 2026-02-22 | Version alignment — v0.2.0 shipped Phase 1+2, roadmap renumbered. |
| 2.2     | 2026-02-18 | FR-023 (commit splitting), competitive matrix update, 118 tests. |
| 2.1     | 2026-02-17 | Enhancement review integration — eval harness, fallback ladder, cancellation contract, streaming trait, golden fixtures. |

</details>

## 1. Vision

> **"The commit message generator that actually understands your code."**

CommitBee is a Rust-native CLI tool that uses tree-sitter semantic analysis and LLMs to generate high-quality conventional commit messages. Unlike every competitor in the market, CommitBee doesn't just send raw `git diff` output to an LLM — it parses both the staged and HEAD versions of files, maps diff hunks to symbol spans (functions, classes, methods), and provides structured semantic context. This architectural advantage produces fundamentally better commit messages, especially for complex multi-file changes.

### Core Principles

1. **Semantic first** — AST analysis is the moat, not an afterthought
2. **Local first** — Ollama default, cloud providers opt-in, secrets never leave the machine unless explicitly configured
3. **Correct first** — No panics, no silent failures, no half-working features
4. **Fast startup** — Sub-200ms to first output, streaming for LLM responses
5. **Graceful degradation** — Works without tree-sitter, without a network, in CI, in git hooks, piped to files
6. **Zero surprise** — Explicit over implicit; debug mode (`--show-prompt`) for full transparency

### Compatibility Policy

| Release | Scope | Breaking Changes |
|---------|-------|------------------|
| v0.2.0  | Stability + polish + providers (Phase 1) | None — config format preserved, no breaking CLI changes |
| v0.3.0  | Differentiation core (splitter enhancements, validation, heuristics) | None |
| v0.3.1  | Patch — default model → `qwen3.5:4b`, subject length validation, `think` config | None |
| v0.4.0  | Feature completion (templates, languages, rename detection, history learning) | None |

## 2. Competitive Landscape

### 2.1 Market Position

| Category             | Key Players                                    | CommitBee Advantage                                             |
|----------------------|------------------------------------------------|-----------------------------------------------------------------|
| AI commit generators | opencommit (7.2K★), aicommits (8K★), aicommit2 | **Only tool with tree-sitter semantic analysis**                |
| Rust commit tools    | rusty-commit, cocogitto, convco                | Semantic analysis + AI generation (cocogitto/convco have no AI) |
| IDE-integrated       | GitHub Copilot, JetBrains AI                   | CLI-first, provider-agnostic, privacy-respecting                |

### 2.2 Unique Differentiators (No Competitor Has These)

1. **Tree-sitter semantic analysis** — Every competitor sends raw diffs to LLMs
2. **Commit splitting** — Detects multi-concern staged changes and splits into separate well-typed commits automatically
3. **Built-in secret scanning** — Only ORCommit[^1] also has this (via external Gitleaks)
4. **Token budget management** with adaptive truncation — Most competitors blindly send full diffs
5. **Streaming output** with cancellation — Most wait for complete response
6. **Prompt debug mode** (`--show-prompt`) — Transparency no one else offers

[^1]: ORCommit (<https://github.com/reacherhq/orcommit>) — a Rust-based commit message generator with Gitleaks integration and interactive regeneration with feedback.

### 2.3 Feature Status vs. Market Expectations

| Feature                                            | Market Expectation            | Status          |
|----------------------------------------------------|-------------------------------|-----------------|
| Cloud LLM providers (OpenAI, Anthropic)            | Universal                     | ✅ v0.2.0       |
| Git hook integration                               | Universal                     | ✅ v0.2.0       |
| Shell completions                                  | Expected for CLI tools        | ✅ v0.2.0       |
| Multiple message generation (pick from N)          | Common (aicommits, aicommit2) | ✅ v0.2.0       |
| Commit splitting (multi-concern detection)         | No competitor has this        | ✅ v0.2.0       |
| Custom prompt/instruction files                    | Growing (Copilot, aicommit2)  | ✅ v0.4.0       |
| Unit/integration tests                             | Non-negotiable for quality    | ✅ 367 tests    |

## 3. Architecture

### 3.1 Resolved Issues

The following critical issues from earlier versions have been resolved:

| Issue | Resolution | Version |
|-------|-----------|---------|
| Symbols extracted but never included in LLM prompt | Included in prompt with fallback ladder | v0.2.0 |
| `App::generate_commit()` untestable monolith | Decomposed into testable methods | v0.2.0 |
| No dependency injection | Trait abstractions for GitService, LlmProvider | v0.2.0 |
| Synchronous `std::process::Command` in async runtime | `tokio::process::Command` (FR-020) | v0.2.0 |
| N+1 git process spawns | Single diff + concurrent `JoinSet` (FR-021) | v0.2.0 |
| UTF-8 panic in sanitizer | `str::chars()` safe truncation (FR-001) | v0.2.0 |

### 3.2 Open Architecture Concerns

#### Symbol Extraction Fallback Ladder

When building the LLM prompt, symbol context uses a tiered approach:

1. **AST mapping** — Tree-sitter parses both HEAD and staged versions, maps diff hunks to symbol spans (best quality)
2. **Hunk heuristic** — If tree-sitter grammar unavailable, extract nearest function/class from hunk header (`@@ ... @@ fn name`)
3. **File summary** — If hunk heuristic fails, include file-level summary (path, change status, line counts)
4. **Raw diff** — Final fallback, plain diff with no semantic annotation

Each tier produces progressively less useful context but ensures the pipeline never blocks on a parse failure.

#### Dependency Status

| Dependency | Status | Notes |
|------------|--------|-------|
| `anyhow` | ✅ Removed | Never imported |
| `once_cell` | ✅ Replaced | `std::sync::LazyLock` (stable since Rust 1.80) |
| `async-trait` | ✅ Replaced | Native async traits (edition 2024) |
| `futures` | ✅ Replaced | `tokio-stream` `StreamExt` |
| `miette` | ✅ Added | Rich diagnostic errors |
| `figment` | ✅ Added | Hierarchical config |
| `tracing` + `tracing-subscriber` | ✅ Added | Structured logging |
| `clap_complete` | ✅ Added | Shell completions |
| `keyring` | ✅ Added | Secure API key storage |
| `insta` | ✅ Added (dev) | Snapshot testing |
| `indicatif` | ✅ Active | Progress indicators |

### 3.3 Target Architecture

```
commitbee
├── src/
│   ├── main.rs              # Entry point (uses lib, not mod declarations)
│   ├── lib.rs               # #![forbid(unsafe_code)] + public API
│   ├── cli.rs               # clap derive with ValueEnum, subcommands
│   ├── config.rs            # figment-based hierarchical config
│   ├── error.rs             # miette diagnostics + thiserror
│   ├── app.rs               # Orchestrator (decomposed into small methods)
│   ├── domain/
│   │   ├── change.rs        # FileChange, StagedChanges
│   │   ├── symbol.rs        # CodeSymbol, SymbolKind, SymbolChangeType
│   │   ├── context.rs       # PromptContext (includes symbols in prompt)
│   │   └── commit.rs        # CommitType (single source of truth for types)
│   ├── queries/             # Tree-sitter .scm query files per language
│   └── services/
│       ├── git.rs           # GitService trait + impl (async, single-diff)
│       ├── analyzer.rs      # AnalyzerService (parallel parsing via rayon)
│       ├── context.rs       # ContextBuilder (fixed budget math, fallback ladder)
│       ├── safety.rs        # Secret scanning (24 patterns, pluggable engine)
│       ├── sanitizer.rs     # CommitSanitizer (UTF-8 safe) + CommitValidator (7 rules)
│       ├── splitter.rs      # CommitSplitter (Jaccard + fingerprinting)
│       ├── template.rs      # TemplateService (custom prompt templates)
│       ├── history.rs       # HistoryService (commit style learning, experimental)
│       └── llm/
│           ├── mod.rs       # LlmProvider trait (native async, enum dispatch)
│           ├── ollama.rs    # Ollama (timeout, error differentiation)
│           ├── openai.rs    # OpenAI-compatible (OpenAI, Groq, Together, LM Studio, vLLM)
│           └── anthropic.rs # Anthropic Claude
├── tests/
│   ├── snapshots/           # insta snapshot files
│   ├── fixtures/            # Eval fixtures (36 scenarios), diff samples
│   ├── helpers.rs           # Shared test helpers (make_file_change, make_staged_changes)
│   ├── context.rs           # ContextBuilder, type inference, evidence, signatures, connections
│   ├── sanitizer.rs         # CommitSanitizer + CommitValidator (unit + snapshot + proptest)
│   ├── splitter.rs          # CommitSplitter grouping and merge logic
│   ├── languages.rs         # Feature-gated per-language symbol + signature extraction
│   ├── safety.rs            # Secret scanning patterns + conflict detection
│   ├── integration.rs       # LLM provider round-trips with wiremock
│   ├── history.rs           # HistoryService with tempfile git repos
│   ├── template.rs          # TemplateService custom/default templates
│   ├── commit_type.rs       # CommitType parsing and ALL sync
│   └── eval.rs              # Eval harness fixture validation (feature-gated)
├── fuzz/                    # cargo-fuzz targets (sanitizer, safety, diff parser, signature, classify_span)
└── completions/             # Generated shell completions
```

### 3.4 Trait Design for Testability

```rust
// Services defined as traits for mockability
pub trait GitOperations: Send + Sync {
    async fn get_staged_changes(&self) -> Result<StagedChanges>;
    async fn get_file_diff(&self, path: &Path) -> Result<String>;
    async fn fetch_file_contents(&self, paths: &[PathBuf]) -> (HashMap<PathBuf, String>, HashMap<PathBuf, String>);
    async fn commit(&self, message: &str) -> Result<()>;
}

pub trait CodeAnalyzer: Send + Sync {
    fn extract_symbols(&self, changes: &[FileChange], staged: &HashMap<PathBuf, String>, head: &HashMap<PathBuf, String>) -> Vec<CodeSymbol>;
}

// LlmProvider with native async (no async_trait)
// Both generate() (buffered) and generate_stream() (streaming) required.
pub trait LlmProvider: Send + Sync {
    async fn generate(&self, system: &str, user: &str, cancel: CancellationToken) -> Result<String>;
    async fn generate_stream(
        &self,
        system: &str,
        user: &str,
        cancel: CancellationToken,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>>;
    fn name(&self) -> &str;
    fn supports_streaming(&self) -> bool;
}

// App takes trait objects for testability
pub struct App {
    git: Box<dyn GitOperations>,
    analyzer: Box<dyn CodeAnalyzer>,
    llm: Box<dyn LlmProvider>,
    config: Config,
}
```

`generate_stream()` is required for all providers. Providers that do not support streaming implement it by wrapping `generate()` as a single-element stream.

## 4. Feature Requirements

### 4.1 Shipped — v0.2.0 (Stability & Providers)

All features in this section shipped in v0.2.0. Included for completeness and traceability.

#### FR-001: Fix UTF-8 Panics in Sanitizer ✅

Use `str::chars().take(69).collect::<String>()` for safe truncation. Proptest guarantees sanitizer never panics on arbitrary Unicode input.

#### FR-002: Include Symbols in LLM Prompt ✅

`to_prompt()` includes a "Symbols changed" section using the fallback ladder (AST mapping → hunk heuristic → file summary → raw diff). Graceful degradation when tree-sitter parsing fails.

#### FR-003: Unit Test Suite ✅

Snapshot tests (insta) for sanitizer, diff parser, safety scanner, context builder, and file categorizer. Proptest for never-panic guarantees.

#### FR-004: Remove Unused Dependencies ✅

Removed `anyhow`, replaced `once_cell` with `std::sync::LazyLock`, replaced `async-trait` with native async traits, replaced `futures` with `tokio-stream`.

#### FR-005: Fix Dead Code ✅

All dead fields either implemented (rename detection, signature display) or removed. No compiler warnings.

#### FR-006: Reduce Tokio Features ✅

Features reduced to `["rt-multi-thread", "macros", "signal", "sync", "process"]`.

#### FR-007: CommitType Single Source of Truth ✅

`CommitType` provides `const ALL: &[&str]` used by sanitizer and validation. Compile-time test ensures sync.

#### FR-010: Rich Diagnostic Errors (miette) ✅

Every error variant has a human-readable message, error code, help suggestion, and source context where applicable.

#### FR-011: OpenAI-Compatible Provider ✅

Supports any OpenAI-compatible API (OpenAI, Groq, Together, LM Studio, vLLM). Configurable `api_base_url`, `model`, `api_key`. Streaming with cancellation. Tested with wiremock.

#### FR-012: Anthropic Provider ✅

Native Anthropic Claude API support. Streaming via `generate_stream()` with cancellation. Tested with wiremock.

#### FR-013: Ollama Hardening ✅

Configurable timeout (default 300s), connection/model error differentiation with help text, configurable `temperature`/`num_predict`, health check, mid-stream error handling, streaming support.

#### FR-014: Git Hook Integration ✅

`commitbee hook install/uninstall/status`. Non-destructive (backs up existing hooks). Detects and skips merge/amend/squash commits. Atomic writes. Graceful fallback if binary not found.

#### FR-015: Shell Completions ✅

`commitbee completions <shell>` for bash, zsh, fish, powershell via `clap_complete`.

#### FR-016: Multiple Message Generation ✅

`commitbee --generate N` with `dialoguer` interactive selection in TTY mode. Non-TTY outputs all N. `--yes` auto-selects first.

#### FR-017: Hierarchical Configuration (figment) ✅

Priority: CLI args > env vars > project config (`.commitbee.toml`) > user config > defaults.

| Platform | User Config Path |
|----------|-----------------|
| macOS    | `~/Library/Application Support/commitbee/config.toml` |
| Linux    | `~/.config/commitbee/config.toml` (XDG) |
| Windows  | `%APPDATA%\commitbee\config.toml` |

Fallback: `~/.config/commitbee/config.toml` on all platforms for backward compatibility.

#### FR-018: Structured Logging (tracing) ✅

`RUST_LOG=commitbee=debug` for verbose output. `--verbose` / `-v` flag. Key functions instrumented with `#[instrument]`.

#### FR-019: Secure API Key Storage ✅

System keychain via `keyring` (feature-gated). `commitbee config set-key/get-key <provider>`. Env var fallback. Never stores keys in plaintext config.

#### FR-020: Async Git Operations ✅

All git CLI calls use `tokio::process::Command`. Event loop never blocked.

#### FR-021: Single-Pass Diff Parsing ✅

One `git diff --cached --no-ext-diff --unified=3` call parsed per-file. NUL-delimited name-status parsing (`-z` flag).

#### FR-022: Integration Test Suite ✅

End-to-end tests with `tempfile` git repos and `wiremock` LLM mocks. CLI tests with `assert_cmd`/`insta-cmd`.

#### FR-023: Commit Splitting ✅

Detects logically independent staged changes and splits into separate well-typed commits.

**Implementation details:**
1. Diff-shape fingerprinting + Jaccard clustering (vocabulary overlap > 0.4)
2. Symbol dependency merging via targeted caller detection
3. Category separation (docs/config get own groups)
4. Module detection with 22-entry generic directory exclusion list
5. Post-clustering sub-split for >6-file groups spanning multiple modules
6. Scored support file assignment (known pairs, stem overlap, standalone fallback)
7. Type+scope inference per group
8. Group rationale in per-group prompts (`GROUP_REASON:`)
9. Focus instruction for >5-file groups
10. Collapse check (same type+scope → suggest single commit)
11. Split execution: unstage all → stage group → commit → repeat

**Safety**: Refuses to split when staged files also have unstaged modifications.
**CLI**: `--no-split` disables. `--yes` and non-TTY skip suggestion (default single commit).
**Tests**: 16 dedicated integration tests.

#### FR-039: Config Validation ✅

`commitbee config check` validates configuration. `commitbee doctor` checks Ollama health. URL parsing, numeric bounds, provider enum validation at config time.

### 4.2 Shipped — v0.3.x (Differentiation Core)

Features that shipped incrementally across v0.3.0 and v0.3.1.

#### FR-034: Improved Commit Type Heuristics ✅

Evidence-based deterministic commit type inference:

- Test-only → `test`, doc-only → `docs`, CI-only → `ci`, dependency-only → `chore`
- New files with substantial code → `feat`
- `fix` requires `has_bug_evidence` (bug-fix comments in diff); without → `refactor`
- API replacement detection (public APIs added AND removed → `refactor`)
- Mechanical/formatting detection → `style`/`refactor` (never `feat`/`fix`)
- Metadata-aware breaking detection: `rust-version`, `engines.node`, `requires-python` changes; removed `pub use`/`pub mod`/`export`
- Symbol tri-state: `AddedOnly`, `RemovedOnly`, `ModifiedSignature` — public modified symbols contribute to breaking risk
- Default fallback: `Refactor` (safer than `Feat` for ambiguous changes)

#### FR-040: Conventional Commits 1.0.0 Spec Anchoring ✅

- Breaking changes: `!` suffix + `BREAKING CHANGE:` footer (always emitted, regardless of `include_body`)
- Footer wrapped at 72 chars with 2-space continuation indent (git-trailer compatible)
- Single shared `SYSTEM_PROMPT` constant; type list synced with `CommitType::ALL` via compile-time test
- Sanitizer normalizes `"null"` → non-breaking
- Symbol deduplication in context builder
- Cross-project file categorization: 30+ source language extensions, 40+ config patterns, dotfile auto-detection, expanded CI/build/lock file detection
- Expanded scope inference: additional source/monorepo dirs, generic next-component exclusion

#### FR-041: Post-Generation Validation ✅

Evidence-based LLM output validation with multi-pass corrective retry (up to 3 attempts).

**Evidence flags** (computed before LLM generation):
- `is_mechanical`, `has_bug_evidence`, `public_api_removed_count`, `has_new_public_api`, `is_dependency_only`

**CommitValidator — 7 rules:**
1. `fix` requires `has_bug_evidence` (otherwise → `refactor`)
2. `breaking_change` required when public APIs removed
3. `breaking_change` must not copy internal field names (anti-hallucination)
4. Mechanical transforms cannot be `feat`/`fix` (→ `style`/`refactor`)
5. Dependency-only changes must be `chore`
6. Subject specificity: generic verb+noun triggers retry with instruction to name specific APIs/modules
7. Subject length: rejects subjects exceeding 72-char first line, reports char budget

**Retry behavior** (v0.3.1): Appends `CORRECTIONS` section, re-prompts, re-validates. Sanitizer rejects overlong first lines with descriptive error (no silent truncation).

### 4.3 Shipped — v0.4.0 (Feature Completion)

#### FR-030: Custom Prompt Templates ✅

`TemplateService` in `src/services/template.rs`. Config fields: `system_prompt_path`, `template_path`. Template variables: `{{diff}}`, `{{symbols}}`, `{{files}}`, `{{type}}`, `{{scope}}`. Default templates used when no custom template specified. All LLM providers pass through custom system prompt. 7 tests.

#### FR-032: Multi-Language Commit Messages ✅

`--locale <lang>` flag and `locale` config option. `LANGUAGE:` instruction injected into prompt context. ISO 639-1 codes supported. Type/scope remain in English per conventional commits spec.

#### FR-035: Rename Detection ✅

`--find-renames=N%` with configurable `rename_threshold` (default 70%, 0 disables). NUL-delimited `R<NNN>` status parsing consuming two path fields. `ChangeStatus::Renamed` variant with `old_path` on `FileChange`. Context builder formats as `old → new (N% similar)`. Split suggestions show `[R]` marker.

#### FR-036: Tree-sitter Query Patterns ✅

`.scm` query files in `src/queries/` per language with `@name`/`@definition` captures. `LanguageConfig` with `query_source` field. `tree_sitter::Query` + `QueryCursor` + `StreamingIterator` replaces manual `TreeCursor` walking.

#### FR-037: Expanded Secret Scanning ✅

25 built-in `SecretPattern` structs across 13 categories:

| Category | Patterns |
|----------|----------|
| Cloud | AWS access/secret, GCP service account/API key, Azure storage |
| AI/ML | OpenAI (`sk-proj-`), Anthropic, HuggingFace |
| Source Control | GitHub PAT/fine-grained/OAuth, GitLab |
| Communication | Slack token/webhook, Discord webhook |
| Payment | Stripe, Twilio, SendGrid, Mailgun |
| Database | Connection strings |
| Crypto | Private keys, JWT |
| Generic | API key patterns (quoted/unquoted) |

Pluggable engine via `build_patterns(custom, disabled)`. Config: `custom_secret_patterns`, `disabled_secret_patterns`. `LazyLock` default pattern set.

#### FR-038: Progress Indicators ✅

`Progress` struct wrapping `Option<ProgressBar>` with TTY detection (`std::io::stderr().is_terminal()`). Methods: `phase()`, `info()`, `warning()`, `finish()`. `Drop` auto-clears. Non-TTY falls back to `eprintln!` with `console::style()`.

#### FR-057: Additional Language Support ✅

5 new language crates as optional dependencies: `tree-sitter-java`, `tree-sitter-c`, `tree-sitter-cpp`, `tree-sitter-ruby`, `tree-sitter-c-sharp`. Feature flags: `lang-java`, `lang-c`, `lang-cpp`, `lang-ruby`, `lang-csharp`, `all-languages`. Each language has `.scm` query files. Visibility detection for Java/C# public modifiers. 15 feature-gated tests.

#### FR-058: Commit History Style Learning (Experimental) ✅

`HistoryService` in `src/services/history.rs`. `analyze()` fetches last N commit subjects via `git log`, extracts type distribution, scope patterns, case style, conventional compliance ratio, sample subjects. `HistoryContext::to_prompt_section()` formats as `PROJECT STYLE` block.

Config: `learn_from_history` (default `false`), `history_sample_size` (default 50). Feature-gated behind `--experimental-history` or config flag. Does not override conventional commits structure — only influences scope naming and subject phrasing style. Deterministic sort order for equal-count entries.

#### TR-006: Evaluation Harness ✅

`commitbee eval` — runs full pipeline against fixture diffs with assertion-based validation. Feature-gated (`eval` feature). 36 fixtures in `tests/fixtures/eval/` covering all 11 commit types, AST features (signatures, connections, whitespace classification), and edge cases. Each fixture has `metadata.toml` (assertions for type, evidence flags, prompt content, connections, breaking changes), `diff.patch`, and optional `symbols.toml` (injected CodeSymbol data). `EvalSummary` reports per-type accuracy and overall score. `run_sync()` method for integration test access.

#### TR-007: Fuzzing ✅

5 `cargo-fuzz` targets: `fuzz_sanitizer`, `fuzz_safety`, `fuzz_diff_parser`, `fuzz_signature`, `fuzz_classify_span`. `fuzz/Cargo.toml` with `libfuzzer-sys`.

#### FR-031: Exclude Files ✅

`--exclude` CLI flag (repeatable) and `exclude_patterns` config option. Glob patterns via `globset` (e.g., `*.lock`, `**/*.generated.*`, `vendor/**`). Excluded files listed in output but not analyzed or included in diff context. CLI patterns additive with config patterns. Returns `NoStagedChanges` if all files excluded. 4 glob matching tests + 3 TOML tests + 3 CLI parsing tests.

#### FR-033: Copy to Clipboard ✅

`--clipboard` flag copies generated message to system clipboard and prints to stdout. Skips commit confirmation prompt. Uses platform-specific commands: `pbcopy` (macOS), `clip` (Windows), `xclip -selection clipboard` (Linux). Descriptive error if clipboard command unavailable. 3 CLI parsing tests.

### 4.5 Shipped — v0.5.0 (AST Context Overhaul)

#### FR-059: Full Signature Extraction ✅

Tree-sitter AST nodes now yield complete function/struct/trait signatures (e.g., `pub fn connect(host: &str, timeout: Duration) -> Result<Connection>`) instead of bare names. Two-strategy body detection: `child_by_field_name("body")` primary, `BODY_NODE_KINDS` constant fallback (12 node kinds across 10 languages), first-line final fallback. Multi-line signatures collapsed to single line, capped at 200 chars with UTF-8-safe truncation (`floor_char_boundary`). Token budget rebalanced to 30/70 symbol/diff when signatures present. 7 unit tests + 6 per-language integration tests.

#### FR-060: Semantic Change Classification ✅

Modified symbols (same name+kind+file in both HEAD and staged) are classified as whitespace-only or semantic via character-stream comparison of non-whitespace content within symbol spans. Dual old-file/new-file line tracking for correct span attribution. Old → new signature diffs displayed in prompt (`[~] old_sig → new_sig`). Whitespace-only symbols filtered from modified display. Formatting-only changes auto-detected as `CommitType::Style` when all modified symbols are whitespace-only. `build()` restructured to classify before `infer_commit_type`. 3 tests.

#### FR-061: Cross-File Connection Detection ✅

Scans added diff lines for `symbol_name(` call patterns referencing symbols defined in other changed files. Connections displayed in new `CONNECTIONS:` prompt section (e.g., `validator calls parse() — both changed`). Capped at 5 connections to prevent prompt bloat. SYSTEM_PROMPT updated with connection-aware guidance. 1 test + 1 splitter integration test.

#### FR-062: Security Hardening ✅

Project-level `.commitbee.toml` can no longer override `openai_base_url`, `anthropic_base_url`, or `ollama_host` (SSRF/exfiltration prevention). All 3 streaming LLM providers cap `line_buffer` at `MAX_RESPONSE_BYTES` (1 MB) to prevent unbounded memory growth. `reqwest::Error` display stripped of URLs via `without_url()`. OpenAI secret pattern broadened to `sk-proj-` and `sk-svcacct-` prefixes. `Box::leak` replaced with `Cow<'static, str>` for custom secret pattern names.

#### FR-063: Prompt Optimization for Small Models ✅

Subject character budget accounts for `!` suffix on breaking changes. EVIDENCE section omitted when all flags are default (~200 chars saved). Symbol marker legend added to SYSTEM_PROMPT (`[+] added, [-] removed, [~] modified`). Duplicate JSON schema removed from system prompt. Emoji replaced with text labels (`WARNING:` instead of `⚠`). CONNECTIONS instruction softened for small models. Python tree-sitter queries enhanced with `decorated_definition` support.

### 4.6 Future — v0.6.0+ (Market Leadership)

#### FR-050: MCP Server Mode

Run commitbee as an MCP server for editor integration (VS Code, Cursor, Claude Code). Emerging standard; forward-looking integration.

#### FR-051: Changelog Generation

Generate changelogs from commit history using semantic understanding. Natural extension of commit structure. Competes with git-cliff/cocogitto.

#### FR-052: Multi-Provider Concurrent Generation

Query multiple LLMs simultaneously, let user pick best result. Leverages multi-provider support.

#### FR-053: Interactive Regeneration with Feedback

User can say "make it shorter" / "focus on the API change" after seeing a generated message. Turns one-shot generation into a conversation. Inspired by ORCommit[^1].

#### FR-054: Monorepo Support

Detect monorepo structure, scope commits to affected packages. Required for enterprise adoption.

#### FR-055: Version Bumping

Automatic semantic version bumps based on commit types. Natural extension of conventional commits.

#### FR-056: GitHub Action

Run commitbee in CI to validate or rewrite commit messages. Key differentiator for team adoption.

## 5. Security Requirements

### SR-001: Secret Scanning

- Scan all content sent to LLM, not just `+` diff lines
- 25 built-in patterns across 13 categories (see FR-037)
- Configurable pattern allowlist/blocklist
- Never send detected secrets to any LLM provider
- **Proxy/forwarding protection**: Resolve `ollama_host` to IP, verify loopback (`127.0.0.0/8` or `::1`). Reject non-loopback even if hostname looks local. Log warning on ambiguous resolution.

### SR-002: API Key Management

- System keychain via `keyring` (macOS Keychain, Linux Secret Service, Windows Credential Manager)
- Environment variable fallback
- Never stores keys in plaintext config
- Warns if config file permissions are world-readable

### SR-003: Command Execution Safety

- All subprocess calls via `Command::arg()` (never shell interpolation)
- `--` separator before file paths in all git commands
- LLM output validated before use as commit message
- `#![forbid(unsafe_code)]` in `lib.rs`

### SR-004: Input Validation

- All string truncation uses `char_indices()` or `.chars().take(n)` — never byte indexing
- Config values validated at load time (URL parsing, numeric bounds, enum validation)
- LLM JSON output validated against schema before use

### SR-005: Dependency Auditing

- `cargo audit` in CI
- `cargo deny` for license compliance
- Minimize dependency tree

## 6. Performance Requirements

### PR-001: Startup Time

Cold start to first output: < 200ms (excluding LLM generation). Measured with `hyperfine` in CI. Lazy initialization for tracing-subscriber and tree-sitter grammars.

### PR-002: Git Operations

Single `git diff --cached` call parsed per-file. Async process spawning. Target: 100 staged files in < 2s.

### PR-003: Tree-sitter Parsing

Parallel via rayon (one `Parser` per file per thread). Skip files > 100KB. Cancellation via `parser.set_cancellation_flag()`. Lazy grammar loading. Language detection: file extension primary, shebang fallback. Graceful skip for unrecognized languages.

### PR-004: LLM Generation

Streaming output. Configurable timeout (default 300s). Ctrl+C cancellation with clean cleanup. Health check before generation.

### PR-005: Memory

- Token budget: characters (no tokenizer dependency), `max_context_chars` configurable (default 24K)
- Truncation priority (highest preserved first): symbols > file list > diff hunks
- Parse trees dropped after symbol extraction
- Streaming buffer bounded: `MAX_RESPONSE_BYTES` = 1 MB (all providers)

### PR-006: Binary Size

Feature-gated language support. `[profile.release]` with `lto = true`, `strip = true`, `codegen-units = 1`. Target: < 15MB with default features.

### PR-007: Cancellation Contract

Ctrl+C at any pipeline point → no partial commit, no leftover temp files. Git commit only after user confirms complete message. Temp files cleaned via RAII.

## 7. UX Requirements

### UX-001: Error Messages

Every error includes **what** went wrong, **why** it happened, and **how** to fix it:

```
x Cannot connect to Ollama at http://localhost:11434

  help: Is Ollama running? Start it with:
        ollama serve
```

```
x No staged changes found

  help: Stage your changes first:
        git add <files>
```

### UX-002: Terminal Output

- Respect `NO_COLOR`
- Spinner during analysis/generation (suppressed non-TTY)
- Streaming LLM output in real-time
- Phase indicators: "Analyzing → Generating → Done"
- ASCII fallback for limited terminals

### UX-003: Non-Interactive Mode

- `--yes` auto-confirms
- Non-TTY detection for hooks/CI
- All output to stderr except commit message (for piping)
- Exit codes: 0 success, 1 error, 2 usage error, 130 interrupted

### UX-004: CLI Design

```
commitbee [OPTIONS]                    # Generate and commit (default)
commitbee --dry-run                    # Generate, print, don't commit
commitbee --yes                        # Generate and auto-commit
commitbee --generate N                 # Generate N options
commitbee --show-prompt                # Debug: show LLM prompt
commitbee --verbose / -v               # Verbose output
commitbee --no-split                   # Disable commit split suggestions
commitbee --no-scope                   # Disable scope in commit messages
commitbee --clipboard                  # Copy to clipboard
commitbee --locale <lang>              # Commit message language (ISO 639-1)
commitbee --find-renames=N%            # Rename detection threshold
commitbee --experimental-history       # Enable commit history style learning

commitbee init                         # Create config file
commitbee config                       # Show configuration
commitbee config check                 # Validate configuration
commitbee config set-key <provider>    # Store API key in keychain
commitbee doctor                       # Check Ollama connectivity + model

commitbee hook install                 # Install git hook
commitbee hook uninstall               # Remove git hook
commitbee hook status                  # Check hook status

commitbee completions <shell>          # Generate shell completions
commitbee eval                         # Run evaluation harness (dev, feature-gated)
```

### UX-005: First-Run Experience

- Zero config with Ollama detected
- Helpful setup guidance if no Ollama and no cloud provider
- `commitbee init` creates well-commented config with all options documented

### UX-006: Output Format Contracts

| Flag | stdout | stderr | Behavior |
|------|--------|--------|----------|
| `--dry-run` | Commit message (single line) | Spinners, diagnostics | Exit 0 |
| `--generate N` (TTY) | Selected message | N numbered options + `dialoguer` prompt | `--yes` selects first |
| `--generate N` (non-TTY) | All N messages, blank-line separated | Diagnostics | — |
| `--show-prompt` | — | Full LLM prompt (keys redacted) | Does not call LLM. Exit 0 |
| Default (interactive) | Commit hash on confirm | Message + confirm/edit/cancel prompt | — |

## 8. Testing Requirements

**Current test count: 367**

### TR-001: Unit Tests

| Module | Technique | Coverage Target |
|--------|-----------|-----------------|
| `CommitSanitizer` | Snapshot (insta) + proptest | All code paths + never-panic guarantee |
| `DiffHunk::parse_from_diff` | Snapshot | Standard diffs, renames, binary, empty |
| `safety::scan_for_secrets` | Unit + proptest | Each pattern + false positive tests |
| `ContextBuilder` | Snapshot | Budget calculation, type inference, scope inference |
| `FileCategory::from_path` | Unit | All categories, edge cases |
| `CommitType` | Unit | Verify `ALL` matches enum variants |
| `CommitValidator` | Unit | All 7 rules, boundary cases, corrections formatting |
| `TemplateService` | Unit | Custom/default templates, variable substitution |
| `HistoryService` | Unit | Style analysis, prompt section formatting |

#### Golden Semantic Fixtures

Stored in `tests/fixtures/golden/` — before/after file pairs with expected diff and symbol extraction output:

- Moved function (diff shows delete + add, symbols show single move)
- Signature change (parameter/return type modified)
- Refactor extract (new function + modified caller)
- Rename symbol (across multiple sites)
- Multi-file change (shared symbol references)

### TR-002: Integration Tests

| Scenario | Setup | Mock |
|----------|-------|------|
| Normal commit flow | tempfile git repo | wiremock Ollama |
| Empty staging area | tempfile git repo | None |
| Binary files mixed with text | tempfile git repo | wiremock Ollama |
| Large diff (truncation) | tempfile git repo | wiremock Ollama |
| Unicode file paths | tempfile git repo | wiremock Ollama |
| LLM returns invalid JSON | tempfile git repo | wiremock Ollama |
| LLM returns error mid-stream | tempfile git repo | wiremock Ollama |
| Ollama not running | None | Real connection refused |
| Secret detected | tempfile git repo | None |
| Non-TTY mode | tempfile + piped stdin | wiremock Ollama |

### TR-003: CLI Tests

Snapshot tests with `insta-cmd` for all flag combinations: `--dry-run`, `--show-prompt`, `--help`, error formatting, exit codes.

### TR-004: Property-Based Tests

```rust
proptest! {
    #[test]
    fn sanitizer_never_panics(s in "\\PC*") {
        let _ = CommitSanitizer::sanitize(&s);
    }

    #[test]
    fn secret_scanner_never_panics(s in "\\PC*") {
        let _ = scan_for_secrets(&s);
    }
}
```

### TR-005: CI Pipeline

- `cargo check` → `cargo clippy -- -D warnings` → `cargo test` → `cargo audit` → `cargo deny check`
- Triggers: push to `development`, all PRs
- Matrix: stable Rust + MSRV 1.94
- Edition 2024 (requires MSRV 1.94; let chains raise effective MSRV to 1.94)

### TR-006: Evaluation Harness ✅

`commitbee eval` — fixture-based pipeline regression testing. Feature-gated. See §4.3.

### TR-007: Fuzzing ✅

3 `cargo-fuzz` targets. See §4.3.

## 9. Distribution Requirements

### DR-001: cargo install

`cargo install commitbee` on all tier-1 platforms. Published on crates.io.

### DR-002: Prebuilt Binaries

GitHub Releases via `cargo-dist`. Platforms: macOS ARM64/x86_64, Linux x86_64/ARM64, Windows x86_64. Shell installer, checksums, GitHub attestations.

### DR-003: Homebrew

`brew install sephyi/tap/commitbee` (generated by `cargo-dist`).

### DR-004: Shell Completions

bash, zsh, fish, powershell via `clap_complete`. Documented installation per shell in README.

### DR-005: Release Profile

```toml
[profile.release]
lto = true
strip = true
codegen-units = 1
opt-level = "z"  # or "s" — benchmark both
```

## 10. Prompt Engineering Requirements

### PE-001: System Prompt

- Defines persona, rules, and output format
- JSON schema template with nullable fields and 2 micro few-shot examples (API replacement, style-only change) optimized for <4B parameter models
- **Concrete entity rule**: Subject must name at least one concrete entity from the diff
- Negative examples (BAD/GOOD pairs): flags vague and multi-concern subjects
- Anti-hallucination rules: "Never copy labels, field names, or evidence tags from the prompt"
- API replacement rule: added + removed public APIs → `refactor`
- Breaking change guidance: only when existing users/dependents must change code, config, or scripts
- Single shared `SYSTEM_PROMPT` constant in `llm/mod.rs`; type list synced with `CommitType::ALL` via compile-time test

### PE-002: User Prompt

- File list with change status, semantic symbols, truncated diff
- Symbols with tri-state: "Added", "Removed", "Modified (signature changed)"
- Suggested type/scope from heuristics (hints, not requirements)
- **Evidence flags**: Natural language labels (not snake_case) to prevent model copying
- **Subject budget**: Exact remaining characters after `type(scope): ` prefix
- **PRIMARY_CHANGE**: Anchors subject to most significant change (new public API > removed > largest file)
- **CONSTRAINTS**: Dynamic rules from evidence (e.g., "No bug-fix comments — do not use type fix")
- **PUBLIC API REMOVED** warning with listed symbols
- **Metadata breaking signals** (MSRV, engines.node, requires-python)
- **GROUP_REASON** per split group
- **Focus instruction** for >5-file groups

### PE-003: Multi-Stage for Large Diffs

When diff exceeds 50% of token budget: two-stage (per-file summary → commit message). Fallback: single-stage with aggressive truncation.

### PE-004: Model-Specific Tuning

- Temperature: 0.0–0.3 (configurable)
- `num_predict` / `max_tokens`: 256 default (configurable)
- Model-appropriate stop sequences
- System prompt complexity scaled to model size

### PE-005: Binary File Handling

Binary files never included as diff content. Listed in file list with change status and size delta (e.g., `+ assets/logo.png (binary, +24KB)`).

### PE-006: JSON Parse Failure Recovery

Invalid JSON → retry once with repair prompt. Second failure → heuristic extraction (type from file categories, first coherent sentence as description). Never retry more than once.

## 11. Roadmap Summary

| Phase | Version | Status | Focus |
|-------|---------|--------|-------|
| 1 | v0.2.0 | ✅ Shipped | Stability, correctness, providers, developer experience |
| 2 | v0.3.x | ✅ Shipped | Differentiation — heuristics, validation, spec compliance |
| 3 | v0.4.0 | ✅ Shipped | Feature completion — templates, languages, rename, history, eval, fuzzing |
| 4 | v0.4.x | ✅ Shipped | Remaining polish — exclude files (FR-031), clipboard (FR-033) |
| 5 | v0.5.0 | ✅ Shipped | AST context overhaul — full signatures, semantic change classification, cross-file connections. 367 tests. |
| 6 | v0.6.0+ | 📋 Planned | Market leadership — MCP server, changelog, monorepo, version bumping, GitHub Action |

## 12. Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Runtime panics | 0 | proptest + fuzzing, no `unwrap()` on user-facing paths |
| Test coverage | > 80% on services/ | `cargo tarpaulin` |
| CI green rate | > 99% | GitHub Actions dashboard |
| Cold startup | < 200ms | `hyperfine` in CI |
| Binary size (default features) | < 15MB | CI artifact size tracking |
| Commit message quality | > 80% "good enough" first try | Manual evaluation + `commitbee eval` |
| Secret leak rate | 0 | Integration tests with known patterns |
| MSRV | Rust 1.94 (edition 2024) | CI matrix (stable + 1.94) |
| Test count | ≥ 308 | `cargo test` (current: 334) |

## 13. Non-Goals

- **GUI/TUI** — CLI-first. Editor integration via MCP server mode.
- **General-purpose code review** — Commit messages only.
- **Git client replacement** — Wraps git for commits, doesn't replace add/push/etc.
- **WASM plugin system** — Configuration-driven extensibility first.
- **Non-git VCS** — Git covers > 95% of the market.
- **Shell snippet detection** — Commit messages never executed by git; standard sanitization sufficient.

## Appendix A: Competitive Feature Matrix

| Feature | commitbee | opencommit | aicommits | aicommit2 | rusty-commit | cocogitto |
|---------|-----------|------------|-----------|-----------|--------------|-----------|
| **Tree-sitter AST** | ✅ | — | — | — | — | — |
| **Commit splitting** | ✅ | — | — | — | — | — |
| **Secret scanning** | ✅ | — | — | — | — | — |
| **Token budget** | ✅ | — | — | — | — | N/A |
| **Streaming** | ✅ | — | — | — | — | N/A |
| **Local LLM** | ✅ | ✅ | ✅ | ✅ | ✅ | N/A |
| **OpenAI** | ✅ | ✅ | ✅ | ✅ | ✅ | N/A |
| **Anthropic** | ✅ | ✅ | — | ✅ | ✅ | N/A |
| **Git hooks** | ✅ | ✅ | ✅ | — | ✅ | ✅ |
| **Multi-generate** | ✅ | ✅ | ✅ | — | — | — |
| **Shell completions** | ✅ | — | — | — | — | ✅ |
| **MCP server** | Planned | — | — | — | ✅ | — |
| **Changelog** | Future | — | — | — | — | ✅ |
| **Version bumping** | Future | — | — | — | — | ✅ |
| **Monorepo** | Future | — | — | — | — | ✅ |

## Appendix B: Research Sources

1. **Codebase analysis** — Line-by-line review of all source files
2. **Competitor analysis** — 30+ tools across TypeScript, Rust, Python, Go
3. **Best practices** — Rust CLI patterns, LLM prompt engineering, tree-sitter techniques, security, testing, distribution
