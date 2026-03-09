<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
-->

# CommitBee — Product Requirements Document

**Version**: 3.0
**Date**: 2026-03-08
**Status**: Active
**Author**: Sephyi + Claude

**Revision 3.0**: v0.3.1 multi-pass retry + prompt enforcement (2026-03-08) — FR-041 expanded to 7 rules: added Rule 7 (subject length validation with char budget). `validate_and_retry` upgraded from single-shot to multi-pass loop (up to 3 attempts with re-validation after each). Prompt engineering: "HARD LIMIT" phrasing + char budget embedded in JSON template placeholder for stronger small-model compliance. 182 tests.

**Revision 2.9**: v0.3.1 patch (2026-03-08) — Default model switched from `qwen3:4b-instruct-2507-q8_0` to `qwen3.5:4b` (smaller, no thinking overhead, clean JSON output). Subject length enforcement: CommitValidator Rule 7 rejects subjects that would exceed 72-char first line and triggers corrective retry with budget hint; sanitizer returns error instead of silent `...` truncation (removed `truncate_with_ellipsis`). Added `think` config option (default: `false`) for Ollama thinking/reasoning separation. 182 tests.

**Revision 2.8**: v0.3.0 release prep (2026-03-08) — Sanitizer robustness: thought/think block stripping (both `<thought>` and qwen3 `<think>` tags), targeted JSON extraction via "type" key, conversational preamble detection via `VALID_TYPE_START_REGEX`, 9 new sanitizer tests (178 total). Splitter: Jaccard similarity hybrid clustering (content vocabulary overlap alongside diff-shape fingerprinting). Prompt engineering: removed Think-then-Compress (caused token budget exhaustion on <10B models), simplified user prompt, integrated concrete entity rule into system prompt. Git service: NUL-delimited name-status parsing (`-z` flag). Safety: component-based path matching in conflict detector, added-line-only scanning, concat! self-detection prevention. Context: bug evidence → fix type inference, default fallback changed from Feat to Refactor. CI: MSRV matrix updated 1.85→1.94.

**Revision 2.7**: Splitter precision + subject quality + metadata breaking (2026-03-08) — FR-023 enhanced: targeted caller detection (E1), post-clustering sub-split for >6-file groups (E2), focus instruction for >5-file groups (E3), scored support file assignment with known pairs (H6), group rationale in per-group prompts (H2). FR-034 now fully implemented: metadata-aware breaking detection for MSRV/engines.node/requires-python (G1), symbol tri-state with ModifiedSignature (H5). FR-041 expanded to 6 rules: added subject specificity validator (F3). PE-001: negative examples (BAD/GOOD pairs). PE-002: primary change detection (F1), metadata breaking signals, modified symbols section. Performance: Arc\<String\> for diffs, String::with_capacity. Test count: 169.

**Revision 2.6**: Message quality overhaul (2026-03-08) — Added FR-041 (post-generation validation), updated FR-034 (type heuristics partially implemented: evidence flags, API replacement inference, mechanical/style detection), updated FR-023 (splitter: diff-shape fingerprinting, symbol dependency merging, category separation, expanded GENERIC_DIRS to 22 entries, 16 tests), updated FR-040 (cross-project file categorization: 30+ source languages, 40+ config files, expanded CI/build detection, lock file skip list). Updated PE-001 (anti-hallucination rules, anti-copy rule, micro few-shot examples) and PE-002 (evidence flags, subject budget, CONSTRAINTS section, natural language labels). Test count: 168.

**Revision 2.5**: PRD structural cleanup (2026-02-22) — Fixed FR placement inconsistencies: FR-039 definition moved from Section 4.3 to Section 4.2 (shipped in v0.2.0); FR-040 placed only in Phase 2 roadmap (ships with v0.3.0, not v0.2.0); FR-024 (P1 number in P3 context) merged back into FR-058 to preserve decade numbering convention.

**Revision 2.4**: Post-v0.2.0 spec anchoring (2026-02-22) — Conventional Commits 1.0.0 spec compliance: `!` suffix on breaking changes, `BREAKING CHANGE:` footer (emitted regardless of `include_body`), commit type list synced with `CommitType::ALL` via compile-time test, symbol deduplication in context builder (prevents misleading LLM when function bodies change but definition lines don't move). Test count: 133.

**Revision 2.3**: Version alignment (2026-02-22) — v0.2.0 shipped containing all Phase 1 (stability) and Phase 2 (polish/providers) features. Roadmap renumbered: Phase 3 (differentiation) is now v0.3.0, Phase 4 (market leadership) is now v0.4.0+.

**Revision 2.2**: Implementation status update + commit splitting (2026-02-18) — added FR-023 (commit splitting), FR-024 (commit history style learning, experimental), updated competitive matrix and roadmap to reflect v0.3.0 features already implemented (OpenAI, Anthropic, hooks, multi-generate, completions, figment config, miette, tracing, single-pass diff, async git, keyring, 118 tests). Updated architecture with `splitter.rs`.

**Revision 2.1**: Enhancement review integration (2026-02-17) — incorporated evaluation harness, symbol extraction fallback ladder, cancellation contract, streaming trait abstraction, golden test fixtures, output format contracts, hook edge cases, JSON retry logic, and 12 additional refinements from verification review.

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

- **v0.2.0** shipped with all Phase 1 (stability) and Phase 2 (polish/providers) features. Config format preserved, no breaking CLI changes.
- **v0.3.0** is the differentiation release. **v0.3.1** is a patch: default model switched to `qwen3.5:4b`, subject length validation with retry, `think` config option. No breaking changes.

## 2. Competitive Landscape

### 2.1 Market Position

| Category             | Key Players                                    | CommitBee Advantage                                             |
| -------------------- | ---------------------------------------------- | --------------------------------------------------------------- |
| AI commit generators | opencommit (7.2K★), aicommits (8K★), aicommit2 | **Only tool with tree-sitter semantic analysis**                |
| Rust commit tools.   | rusty-commit, cocogitto, convco                | Semantic analysis + AI generation (cocogitto/convco have no AI) |
| IDE-integrated       | GitHub Copilot, JetBrains AI                   | CLI-first, provider-agnostic, privacy-respecting                |

### 2.2 Unique Differentiators (No Competitor Has These)

1. **Tree-sitter semantic analysis** — Every competitor sends raw diffs to LLMs
2. **Commit splitting** — Detects multi-concern staged changes and splits into separate well-typed commits automatically. No competitor offers this.
3. **Built-in secret scanning** — Only ORCommit[^1] also has this (via external Gitleaks)
4. **Token budget management** with adaptive truncation — Most competitors blindly send full diffs
5. **Streaming output** with cancellation — Most wait for complete response
6. **Prompt debug mode** (`--show-prompt`) — Transparency no one else offers

[^1]: ORCommit (<https://github.com/reacherhq/orcommit>) — a Rust-based commit message generator with Gitleaks integration and interactive regeneration with feedback.

### 2.3 Gap Status vs. Competitors

| Feature                                                        | Market Expectation            | Current State     |
| -------------------------------------------------------------- | ----------------------------- | ----------------- |
| Cloud LLM providers (OpenAI, Anthropic)                        | Universal                     | **Implemented**   |
| Git hook integration                                           | Universal                     | **Implemented**   |
| Shell completions                                              | Expected for CLI tools        | **Implemented**   |
| Multiple message generation (pick from N)                      | Common (aicommits, aicommit2) | **Implemented**   |
| Unit/integration tests                                         | Non-negotiable for quality    | **169 tests**     |
| Commit splitting (multi-concern detection)                     | No competitor has this        | **Implemented**   |
| Custom prompt/instruction files                                | Growing (Copilot, aicommit2)  | Missing           |

## 3. Architecture Requirements

### 3.1 Current Architecture Assessment

The existing domain/services separation is solid. The pipeline (CLI -> Git -> Analyzer -> Context -> LLM -> Sanitizer -> Commit) is well-conceived. However, several architectural issues must be addressed:

#### Critical Issues

| Issue                                                         | Impact                                          | Resolution                                                  |
| ------------------------------------------------------------- | ----------------------------------------------- | ----------------------------------------------------------- |
| Symbols extracted but never included in LLM prompt            | Tree-sitter analysis is wasted computation      | Include in prompt with fallback ladder                      |
| `App::generate_commit()` is a 160-line untestable monolith    | Cannot unit test any step of the pipeline       | Decompose into testable methods                             |
| No dependency injection                                       | Services hard-wired, can't mock for tests       | Trait abstractions for GitService, LlmProvider              |
| ~~Synchronous `std::process::Command` in async runtime~~      | ~~Blocks tokio event loop on large repos~~      | ✅ Resolved (FR-020: `tokio::process::Command`)             |
| ~~N+1 git process spawns (1 + N per file)~~                   | ~~50 files = 51 process spawns~~                | ✅ Resolved (FR-021: single diff + concurrent `JoinSet`)    |
| UTF-8 panic in sanitizer (byte-index slicing)                 | Runtime crash on emoji/CJK in commit messages   | Use `str::chars()` for safe truncation                      |

#### Symbol Extraction Fallback Ladder

When building the LLM prompt, symbol context uses a tiered approach:

1. **AST mapping** — Tree-sitter parses both HEAD and staged versions, maps diff hunks to symbol spans (best quality)
2. **Hunk heuristic** — If tree-sitter grammar unavailable, extract nearest function/class from hunk header (`@@ ... @@ fn name`)
3. **File summary** — If hunk heuristic fails, include file-level summary (path, change status, line counts)
4. **Raw diff** — Final fallback, plain diff with no semantic annotation

Each tier produces progressively less useful context but ensures the pipeline never blocks on a parse failure.

#### Dependency Cleanup

| Dependency    | Action                                                          | Reason                                                 |
| ------------- | --------------------------------------------------------------- | ------------------------------------------------------ |
| `anyhow`      | **Remove**                                                      | Never imported anywhere                                |
| `indicatif`   | **Keep** (start using)                                          | Declared but never used; needed for progress UX        |
| `once_cell`   | **Replace** with `std::sync::LazyLock`                          | Stable since Rust 1.80, edition 2024                   |
| `async-trait` | **Replace** with native async traits                            | Stable in edition 2024                                 |
| `futures`     | **Replace** with `tokio-stream` `StreamExt`                     | Already a dependency                                   |
| `secrecy`     | **Remove** until cloud providers implemented                    | Wraps unused API key field                             |
| `tokio`       | **Reduce** to `["rt-multi-thread", "macros", "signal", "sync"]` | Pulls unnecessary features                             |

#### New Dependencies

| Dependency                          | Purpose                                                   | Priority |
| ----------------------------------- | --------------------------------------------------------- | -------- |
| `miette`                            | Rich diagnostic errors with help text, codes, suggestions | P0       |
| `figment`                           | Hierarchical config (defaults < file < env < CLI)         | P1       |
| `tracing` + `tracing-subscriber`    | Structured logging/diagnostics                            | P1       |
| `clap_complete`                     | Shell completions generation                              | P1       |
| `keyring`                           | Secure API key storage (macOS Keychain, etc.)             | P1       |
| `insta`                             | Snapshot testing                                          | P0 (dev) |
| `proptest`                          | Property-based testing                                    | P1 (dev) |

### 3.2 Target Architecture

```bash
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
│   │   ├── symbol.rs        # CodeSymbol, SymbolKind
│   │   ├── context.rs       # PromptContext (includes symbols in prompt)
│   │   └── commit.rs        # CommitType (single source of truth for types)
│   └── services/
│       ├── git.rs           # GitService trait + impl (async, single-diff)
│       ├── analyzer.rs      # AnalyzerService (parallel parsing via rayon)
│       ├── context.rs       # ContextBuilder (fixed budget math, fallback ladder)
│       ├── safety.rs        # Secret scanning (expanded patterns)
│       ├── sanitizer.rs     # CommitSanitizer (UTF-8 safe, body wrapping) + CommitValidator (post-gen validation)
│       ├── splitter.rs      # CommitSplitter (multi-commit detection + grouping)
│       └── llm/
│           ├── mod.rs       # LlmProvider trait (native async, enum dispatch)
│           ├── ollama.rs    # Ollama (timeout, error differentiation)
│           ├── openai.rs    # OpenAI-compatible (covers OpenAI, Groq, Together, LM Studio, vLLM)
│           └── anthropic.rs # Anthropic Claude
├── tests/
│   ├── snapshots/           # insta snapshot files
│   ├── fixtures/            # Test git repos, diff samples, tree-sitter fixtures, golden semantic fixtures
│   ├── sanitizer.rs         # Unit + snapshot + proptest
│   ├── context.rs           # Unit + snapshot
│   ├── safety.rs            # Unit + proptest
│   ├── analyzer.rs          # Unit + snapshot with fixture files
│   ├── git.rs               # Integration with tempfile repos
│   ├── ollama.rs            # Integration with wiremock
│   └── cli.rs               # CLI integration with assert_cmd
└── completions/             # Generated shell completions
```

### 3.3 Trait Design for Testability

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

`generate_stream()` is required for all providers in P1 scope (FR-011, FR-012, FR-013). Providers that do not support streaming should implement `generate_stream()` by wrapping `generate()` as a single-element stream.

## 4. Feature Requirements

### 4.1 P0 — Shipped (v0.2.0: Stability & Correctness)

These are bugs, panics, and missing foundations that must be fixed before any new features.

#### FR-001: Fix UTF-8 Panics in Sanitizer

- **What**: `sanitizer.rs` lines 146/206/210 slice strings at byte index 69. Multi-byte characters (emoji, CJK, accented) at that boundary cause a runtime panic.
- **Acceptance**: Use `str::chars().take(69).collect::<String>()` for safe truncation. Add proptest that sanitizer never panics on arbitrary Unicode input.

#### FR-002: Include Symbols in LLM Prompt

- **What**: Tree-sitter extracts `symbols_added` and `symbols_removed` with budget management, but `PromptContext::to_prompt()` never includes them. The entire semantic analysis pipeline produces output that is thrown away.
- **Acceptance**: `to_prompt()` includes a "Symbols changed" section listing added/modified/removed functions, methods, structs. Symbol extraction uses the fallback ladder (AST mapping -> hunk heuristic -> file summary -> raw diff) to ensure the prompt always gets the best available context. If tree-sitter parsing fails for a file, the pipeline gracefully degrades rather than omitting the file.

#### FR-003: Unit Test Suite (Sanitizer, Safety, Context, Hunk Parser)

- **What**: Zero tests exist despite dev-dependencies being declared.
- **Acceptance**:
  - `CommitSanitizer`: snapshot tests for JSON parsing, plain text parsing, edge cases (empty, unicode, nested quotes, markdown fences). Proptest that it never panics.
  - `DiffHunk::parse_from_diff`: snapshot tests for standard diffs, rename diffs, binary diffs, empty diffs.
  - `safety::scan_for_secrets`: unit tests for each pattern, false positive tests, edge cases.
  - `ContextBuilder::infer_commit_type`: unit tests for each heuristic path.
  - `FileCategory::from_path`: unit tests for all categories.
  - All tests use `insta` for snapshot assertions where applicable.

#### FR-004: Remove Unused Dependencies

- **What**: `anyhow` (never imported), `once_cell` (replace with `std::sync::LazyLock`), `async-trait` (replace with native async traits), `futures` (replace with `tokio-stream` `StreamExt`).
- **Acceptance**: `cargo build` succeeds with these crates removed. Binary size decreases.

#### FR-005: Fix Dead Code

- **What**: `old_path` always None, `signature` never read, `ChangeStatus::Renamed` never constructed, `StagedChanges::is_empty()` never called, `CommitType` variants Style/Perf/Ci/Revert never constructed, `is_binary` always false in stored files.
- **Acceptance**: Either implement the features these fields support (rename detection, signature display) or remove them. No compiler warnings for dead code.

#### FR-006: Reduce Tokio Features

- **What**: `tokio features=["full"]` pulls fs, net, io-util, time, process, parking_lot unnecessarily.
- **Acceptance**: Features reduced to `["rt-multi-thread", "macros", "signal", "sync"]`. Add `"process"` only when git calls are migrated to `tokio::process::Command`.

#### FR-007: CommitType Single Source of Truth

- **What**: `VALID_TYPES: &[&str]` in sanitizer and `CommitType` enum can desync.
- **Acceptance**: `CommitType` provides `const ALL: &[&str]` used by both the sanitizer and any validation logic. No separate string list.

### 4.2 P1 — Shipped (v0.2.0: Polish & Providers)

#### FR-010: Rich Diagnostic Errors (miette)

- **What**: Replace bare `eprintln!` error display with `miette` diagnostics.
- **Acceptance**: Every error variant has:
  - A human-readable message
  - An error code (e.g., `commitbee::ollama::connection_refused`)
  - A help suggestion (e.g., "Is Ollama running? Start it with: `ollama serve`")
  - Source context where applicable (config file parse errors show the offending line)

#### FR-011: OpenAI-Compatible Provider

- **What**: Support any OpenAI-compatible API (OpenAI, Groq, Together, LM Studio, vLLM, Ollama's OpenAI endpoint).
- **Acceptance**:
  - Configurable `api_base_url`, `model`, `api_key`
  - Streaming support via `generate_stream()` with cancellation
  - Timeout configuration
  - Works with at minimum: OpenAI GPT-4o, LM Studio local, Groq
  - Tested with wiremock mocks

#### FR-012: Anthropic Provider

- **What**: Native Anthropic Claude API support.
- **Acceptance**: Works with Claude 3.5 Sonnet and Claude 3 Opus. Streaming via `generate_stream()` with cancellation. Tested with wiremock.

#### FR-013: Ollama Hardening

- **What**: Current Ollama provider has no timeout, no error differentiation, no model parameter tuning.
- **Acceptance**:
  - Configurable request timeout (default 300s)
  - Connection refused -> specific error with help text ("Is Ollama running?")
  - Model not found -> specific error listing available models (`/api/tags`)
  - Configurable `temperature` (default 0.3), `num_predict` (default 256)
  - Health check before generation (`/api/tags` endpoint)
  - Handle error responses mid-stream
  - Verify `ollama_host` is actually localhost before skipping secret check
  - Implement `generate_stream()` trait method for streaming responses

#### FR-014: Git Hook Integration

- **What**: `commitbee hook install` / `commitbee hook uninstall` for `prepare-commit-msg` hook.
- **Acceptance**:
  - Installs a shell script in `.git/hooks/prepare-commit-msg`
  - Non-destructive: backs up existing hook if present
  - Hook runs commitbee in non-interactive mode, writes to `$1` (commit msg file)
  - Detects and skips merge commits, amend commits, message-provided commits, and squash commits
  - Respects `--no-verify`: hook checks `$2` for `message`/`merge`/`squash`/`commit` and exits 0 early
  - Uses atomic writes (write to temp file, then rename) to prevent partial commit message files
  - `commitbee hook status` shows whether hook is installed
  - Graceful fallback: if commitbee binary not found, hook exits 0 (doesn't block commits)

#### FR-015: Shell Completions

- **What**: Generate completions for bash, zsh, fish, powershell.
- **Acceptance**: `commitbee completions <shell>` outputs completions to stdout. Documented installation instructions per shell.

#### FR-016: Multiple Message Generation

- **What**: Generate N candidate messages and let user pick.
- **Acceptance**: `commitbee --generate 3` produces 3 options. Interactive selection in TTY mode using `dialoguer` (already a dependency). In non-TTY mode, outputs all N separated by blank lines. First option auto-selected with `--yes`.

#### FR-017: Hierarchical Configuration (figment)

- **What**: Replace manual TOML parsing with figment for proper layered config.
- **Acceptance**: Priority: CLI args > env vars > project config (`.commitbee.toml`) > user config > defaults. Error messages show which source provided which value.
- **Platform-specific user config paths**:

| Platform | Config Path                                           |
| -------- | ----------------------------------------------------- |
| macOS    | `~/Library/Application Support/commitbee/config.toml` |
| Linux    | `~/.config/commitbee/config.toml` (XDG)               |
| Windows  | `%APPDATA%\commitbee\config.toml`                     |

  Use `dirs` crate for platform detection. Existing `~/.config/commitbee/config.toml` remains supported as a fallback on all platforms for backward compatibility.

#### FR-018: Structured Logging (tracing)

- **What**: Replace ad-hoc `eprintln!` with structured tracing.
- **Acceptance**: `RUST_LOG=commitbee=debug` enables verbose output. `--verbose` / `-v` flag maps to tracing levels. Key functions instrumented with `#[instrument]`. Logs include timing information for performance profiling.

#### FR-019: Secure API Key Storage

- **What**: Use system keychain for API keys instead of requiring environment variables.
- **Acceptance**: `commitbee config set-key <provider>` stores API key in macOS Keychain / Linux Secret Service / Windows Credential Manager via `keyring` crate. Falls back to env var if keychain unavailable. Never stores keys in plaintext config files. `commitbee config get-key <provider>` shows whether a key is stored (not the key itself).

#### FR-020: Async Git Operations

- **What**: Replace blocking `std::process::Command` with `tokio::process::Command`.
- **Acceptance**: All git CLI calls use async process spawning. Event loop is never blocked. Verified with `tokio::time::timeout` test.

#### FR-021: Single-Pass Diff Parsing

- **What**: Replace N+1 git calls with single `git diff --cached` parsed per-file.
- **Acceptance**: One `git diff --cached --no-ext-diff --unified=3` call. Output parsed into per-file diffs. Benchmark shows improvement for 50+ file changes.

#### FR-022: Integration Test Suite

- **What**: End-to-end tests with real git repos and mocked LLM.
- **Acceptance**:
  - Git repo setup with `tempfile` + `git init`
  - Ollama mocked with `wiremock`
  - Tests cover: normal flow, empty staging, binary files, large diffs, unicode paths, LLM errors, LLM malformed output, cancelled generation
  - CLI tests with `assert_cmd` / `insta-cmd`

#### FR-023: Commit Splitting (Multi-Concern Detection)

- **What**: Detect when staged changes contain logically independent changes that should be separate commits. Offer to split automatically with per-group LLM message generation.
- **Status**: **Implemented** (v0.2.0, enhanced post-v0.2.0)
- **How it works**:
  1. **Diff-shape fingerprinting + Jaccard clustering**: Groups files by structural similarity of their diffs combined with content vocabulary overlap (Jaccard index > 0.4). Files must share both similar change shape AND significant token overlap to cluster together, preventing false grouping of unrelated small edits.
  2. **Symbol dependency merging**: Groups connected by targeted caller detection are merged — only when a file's diff adds a line that directly calls a new function from another group (`+` lines containing `sym_name(`), not loose text matches that caused cascading merges from imports.
  3. **Category separation**: Docs and config files get their own groups rather than being dumped on the largest source group.
  4. **Module detection**: Group source files by parent directory (e.g., `src/services/llm/*.rs` → "llm"). Fall back to file stem when parent is generic (22 generic dirs: `src`, `lib`, `services`, `domain`, `utils`, `helpers`, `internal`, `core`, `pkg`, `cmd`, `app`, `api`, `modules`, `components`, `common`, `shared`, `middleware`, `handlers`, `controllers`, `models`, `views`, `routes`).
  5. **Post-clustering sub-split**: Groups with >6 files spanning multiple modules are automatically sub-split by module to prevent mega-groups.
  6. **Scored support file assignment**: Support files (docs, config, tests) assigned via affinity scoring — known pairs (Cargo.toml+Lock, package.json+lock), stem overlap, standalone if weak affinity. Replaces blind "attach to largest group" logic.
  7. **Type+scope inference per group**: Each group gets its own `infer_commit_type()` and `infer_scope()`.
  8. **Group rationale**: Each per-group prompt includes `GROUP_REASON:` explaining why files were grouped (e.g., "mechanical refactor across 7 files").
  9. **Focus instruction**: Groups with >5 files get an explicit instruction to focus the subject on the single most significant change.
  10. **Collapse check**: If all groups have the same type and scope, suggest a single commit instead of splitting.
  11. **Split execution**: Unstage all → stage group files → commit → repeat for each group.
- **Safety**: Refuses to split when any staged file also has unstaged modifications (data loss risk).
- **CLI**: `--no-split` disables the feature. `--yes` and non-TTY mode skip split suggestion (default to single commit).
- **Acceptance**: Tested with 16 dedicated integration tests covering single module, multi-module, all-tests, all-docs, same-type collapse, test attachment, sort order, diff-shape clustering, symbol dependency merging, and category separation.

#### FR-039: Config Validation ✅ (shipped in v0.2.0)

- **What**: Invalid config values only fail at runtime.
- **Acceptance**:
  - `commitbee config check` validates configuration
  - `ollama_host` parsed as URL during config load
  - `max_diff_lines` bounded (10-10000)
  - Provider enum validated at config time, not runtime
  - Ollama health check (`/api/tags`) available as `commitbee doctor`
  - Config file permission warning if world-readable and contains keys

### 4.3 P2 — Next (v0.3.0: Differentiation)

#### FR-030: Custom Prompt Templates

- **What**: User-provided system prompt and prompt template files.
- **Acceptance**:
  - `prompt.system_path` and `prompt.template_path` in config
  - Project-level `.commitbee.toml` overrides user config (team standardization)
  - Template variables: `{{diff}}`, `{{symbols}}`, `{{files}}`, `{{type}}`, `{{scope}}`
  - Default templates remain if no custom template specified

#### FR-031: Exclude Files

- **What**: Skip certain files from analysis.
- **Acceptance**: `--exclude` CLI flag and `exclude_patterns` config option. Glob patterns (e.g., `*.lock`, `**/*.generated.*`). Excluded files still listed but not analyzed or included in diff context.

#### FR-032: Multi-Language Commit Messages

- **What**: Generate commit messages in languages other than English.
- **Acceptance**: `--locale <lang>` flag (e.g., `--locale de`, `--locale ja`). Prompt instructs LLM to write in target language. Type/scope remain in English (conventional commits spec).

#### FR-033: Copy to Clipboard

- **What**: `--clipboard` flag copies generated message to clipboard instead of committing.
- **Acceptance**: Uses system clipboard (pbcopy on macOS, xclip/xsel on Linux, clip on Windows). Works in combination with `--dry-run`.

#### FR-034: Improved Commit Type Heuristics ✅ (implemented post-v0.2.0)

- **What**: Deterministic commit type inference with evidence-based gating and metadata-aware breaking detection.
- **Status**: **Implemented** (post-v0.2.0)
- **How it works**:
  - Test-only changes → `test` ✅
  - Doc-only changes → `docs` ✅
  - CI file changes → `ci` ✅
  - New files with substantial code → `feat` ✅
  - Evidence-based `fix` gating: `fix` type requires `has_bug_evidence` (bug-fix comments in diff); without evidence, falls back to `refactor` ✅
  - API replacement detection: when new public APIs added AND old public APIs removed → `refactor` (not `feat`) ✅
  - Mechanical/formatting transform detection: style-only or mechanical changes → `style`/`refactor` (never `feat`/`fix`) ✅
  - Dependency-only detection: all changes in dependency/config files → `chore` ✅
  - Metadata-aware breaking detection: scans diffs for `rust-version` changes (MSRV), `engines.node` tightening, `requires-python` tightening, removed `pub use`/`pub mod`/`export` statements ✅
  - Symbol tri-state classification: `AddedOnly`, `RemovedOnly`, `ModifiedSignature` — same-name add+remove pairs recognized as signature changes, public modified symbols contribute to breaking risk ✅
  - Bug evidence detection: explicit `has_bug_evidence` check early in inference chain → `fix` type when bug-fix comments found ✅
  - Default fallback is `Refactor` (safer than `Feat` for ambiguous changes) ✅

#### FR-035: Rename Detection

- **What**: Detect file renames instead of showing as add + delete.
- **Acceptance**: Use `git diff --cached --find-renames`. Parse `R` status. Set `old_path` field. LLM prompt says "renamed X to Y" instead of "added Y, deleted X".

#### FR-036: Tree-sitter Query Patterns

- **What**: Replace manual AST walking with tree-sitter query S-expressions.
- **Acceptance**: Each language has a `.scm` query file defining symbol extraction. More maintainable, more precise, easier to add new languages.

#### FR-037: Expanded Secret Scanning

- **What**: Current patterns are incomplete.
- **Acceptance**:
  - Updated OpenAI key pattern (`sk-proj-...`)
  - GitHub token patterns (`ghp_`, `gho_`, `ghs_`, `ghu_`, `github_pat_`)
  - AWS access keys (`AKIA...`)
  - Stripe keys (`sk_live_...`, `pk_live_...`)
  - Generic high-entropy string detection in assignment contexts
  - Configurable: users can add custom patterns or disable checks
  - Scan context lines sent to LLM, not just `+` lines

#### FR-038: Progress Indicators

- **What**: No visual feedback during tree-sitter analysis or LLM model loading.
- **Acceptance**: Spinner during "Analyzing code..." and "Generating message..." phases using `indicatif`. Suppressed in non-TTY mode. Respects `NO_COLOR`.

#### FR-040: Conventional Commits 1.0.0 Spec Anchoring ✅ (implemented post-v0.2.0)

- **What**: Full compliance with the Conventional Commits 1.0.0 specification for breaking changes and type list integrity.
- **Acceptance**:
  - Breaking changes emit `!` suffix on the commit first line (e.g., `feat!: remove v1 API`)
  - `BREAKING CHANGE:` footer always emitted for breaking changes regardless of `include_body` config (it is machine-readable metadata, not prose)
  - Footer wrapped at 72 chars with continuation lines indented two spaces (git-trailer compatible)
  - Single shared `SYSTEM_PROMPT` constant in `llm/mod.rs` used by all providers; commit type list kept in sync with `CommitType::ALL` via compile-time test
  - Sanitizer normalizes string literal `"null"` → non-breaking (defensive handling for model template quirk)
  - Symbol deduplication in context builder: functions modified in-place no longer appear as both Added and Removed, preventing misleading LLM context
  - Cross-project file categorization: 30+ source language extensions (Rust, TS, JS, Python, Go, C, C++, C#, Ruby, Swift, Scala, Elixir, PHP, R, Lua, Zig, Nim, Dart, Vue, Svelte, OCaml, Haskell, Clojure, Erlang, Perl, shell), 40+ config file patterns (biome.json, deno.json, .eslintrc, .prettierrc, ruff.toml, Pipfile, Gemfile, pom.xml, build.gradle, mix.exs, pubspec.yaml, renovate.json, dependabot.yml, etc.), dotfile auto-detection (`.something.json/yaml/toml` → Config), expanded CI/build detection (GitLab CI, CircleCI, Jenkinsfile, Travis, Azure Pipelines, Netlify, Vercel, CMake, Procfile)
  - Expanded scope inference: additional source dirs (`app/`, `internal/`, `cmd/`, `api/`, `modules/`), monorepo dirs (`packages/`, `services/`, `plugins/`, `workspaces/`), generic next-component exclusion (`index`)
  - Expanded lock file skip list: `Pipfile.lock`, `uv.lock`, `pubspec.lock`, `flake.lock`, `shrinkwrap.yaml`, `mix.lock` (in addition to existing `Cargo.lock`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `composer.lock`, `Gemfile.lock`, `poetry.lock`, `go.sum`)

#### FR-041: Post-Generation Validation ✅ (implemented post-v0.2.0)

- **What**: Evidence-based validation of LLM output against deterministic code analysis signals, with multi-pass corrective retry on violation.
- **Status**: **Implemented** (post-v0.2.0, enhanced v0.3.1)
- **How it works**:
  1. **Evidence flags**: Five deterministic signals computed from code analysis before LLM generation: `is_mechanical` (formatting/whitespace-only), `has_bug_evidence` (bug-fix comments in diff), `public_api_removed_count` (removed public functions/structs/traits), `has_new_public_api` (new public symbols added), `is_dependency_only` (all changes in dependency/config files).
  2. **CommitValidator**: After LLM generates a structured commit, validates it against evidence flags with 7 rules:
     - `fix` type requires `has_bug_evidence` (otherwise → `refactor`)
     - `breaking_change` must be set when public APIs removed
     - `breaking_change` must not copy internal field names (anti-hallucination)
     - Mechanical transforms cannot be `feat` or `fix` (→ `style`/`refactor`)
     - Dependency-only changes must be `chore`
     - Subject specificity: generic verb+noun combinations (e.g., "update code", "improve things") trigger retry with instruction to name specific APIs/modules changed
     - Subject length: rejects subjects that would produce a first line exceeding 72 chars, reports char budget
  3. **Multi-pass corrective retry** (v0.3.1): On violation, appends a `CORRECTIONS` section to the prompt listing each violation with fix instructions, and re-prompts the LLM. Re-validates the retry output and retries again if violations persist, up to 3 total attempts. Sanitizer rejects overlong first lines with a descriptive error (no silent truncation).
- **Acceptance**: Tested with 10 dedicated unit tests covering each validation rule, valid commit acceptance, boundary cases, and corrections formatting.

### 4.4 P3 — Future (v0.4.0+: Market Leadership)

#### FR-050: MCP Server Mode

- **What**: Run commitbee as an MCP server for editor integration (VS Code, Cursor, Claude Code).
- **Rationale**: Emerging standard (rusty-commit is the only competitor with this). Forward-looking integration strategy.

#### FR-051: Changelog Generation

- **What**: Generate changelogs from commit history using semantic understanding.
- **Rationale**: Natural extension of understanding commit structure. Competes with git-cliff/cocogitto.

#### FR-052: Multi-Provider Concurrent Generation

- **What**: Query multiple LLMs simultaneously, let user pick best result.
- **Rationale**: Innovative UX from aicommit2. Leverages commitbee's multi-provider support.

#### FR-053: Interactive Regeneration with Feedback

- **What**: After seeing generated message, user can say "make it shorter" / "focus on the API change" / "add more detail about the refactor" and regenerate.
- **Rationale**: Innovative UX from ORCommit[^1]. Turns one-shot generation into a conversation.

#### FR-054: Monorepo Support

- **What**: Detect monorepo structure, scope commits to affected packages.
- **Rationale**: cocogitto has excellent monorepo support. Enterprise adoption requires this.

#### FR-055: Version Bumping

- **What**: Automatic semantic version bumps based on commit types.
- **Rationale**: Natural extension of conventional commits understanding. Competes with cocogitto/convco.

#### FR-056: GitHub Action

- **What**: Run commitbee in CI to validate or rewrite commit messages.
- **Rationale**: opencommit's GitHub Action is a key differentiator for team adoption.

#### FR-057: Additional Language Support

- **What**: Expand tree-sitter beyond Rust/TS/JS/Python/Go to Java, C/C++, Ruby, C#, Swift, Kotlin.
- **Acceptance**: Feature-gated language support to control binary size:

  ```toml
  [features]
  default = ["lang-rust", "lang-typescript", "lang-javascript", "lang-python", "lang-go"]
  lang-java = ["tree-sitter-java"]
  lang-cpp = ["tree-sitter-cpp"]
  all-languages = ["lang-java", "lang-cpp", "lang-ruby", "lang-csharp", ...]
  ```

#### FR-058: Commit History Style Learning (Experimental)

- **What**: Analyze existing commit history in the repository to learn the project's commit style, then align generated messages accordingly. This includes scope naming conventions, type usage patterns, subject phrasing style, and body conventions.
- **Status**: Planned (experimental — may diverge from strict Conventional Commits compliance)
- **Rationale**: GitHub Copilot does this implicitly. Making it explicit and configurable would be a differentiator. However, blindly mimicking a repository's history could produce non-compliant messages if the history is inconsistent.
- **Acceptance**: Feature-gated behind `--experimental-history` or a config flag. Samples last N commits, extracts patterns, injects as additional context in the LLM prompt. Does not override conventional commits structure — only influences scope naming and subject phrasing style.

## 5. Security Requirements

### SR-001: Secret Scanning (Enhanced)

- Scan all content sent to LLM, not just `+` diff lines
- Updated patterns for modern API key formats (OpenAI `sk-proj-`, GitHub `ghp_`/`gho_`/`ghs_`/`ghu_`/`github_pat_`, AWS `AKIA`, Stripe `sk_live_`/`pk_live_`)
- Configurable pattern allowlist/blocklist
- Never send detected secrets to any LLM provider, regardless of provider type
- Verify `ollama_host` resolves to localhost before treating as "local" (don't rely on provider enum alone)
- **Proxy/forwarding protection**: Resolve `ollama_host` to an IP address and verify it is a loopback address (`127.0.0.0/8` or `::1`). Reject hostnames that resolve to non-loopback addresses even if the hostname looks local (e.g., `localhost` mapped to a remote IP via `/etc/hosts` or DNS). Log a warning when the resolved address is ambiguous.

### SR-002: API Key Management

- System keychain integration via `keyring` crate (macOS Keychain, Linux Secret Service, Windows Credential Manager)
- Environment variable fallback
- Never store API keys in plaintext config files
- Warn if config file permissions are world-readable

### SR-003: Command Execution Safety

- All subprocess calls via `Command::arg()` (never shell interpolation)
- Use `--` separator before file paths in all git commands
- Validate that LLM output is safe before using as commit message (no shell injection via commit message)
- `#![forbid(unsafe_code)]` in `lib.rs`

### SR-004: Input Validation

- All string truncation uses `char_indices()` or `.chars().take(n)` — never byte indexing
- Config values validated at load time (URL parsing, numeric bounds, enum validation)
- LLM JSON output validated against schema before use

### SR-005: Dependency Auditing

- `cargo audit` in CI pipeline
- `cargo deny` for license compliance
- Minimize dependency tree (remove unused crates)

## 6. Performance Requirements

### PR-001: Startup Time

- Cold start to first output: < 200ms (excluding LLM generation)
- Measured with `hyperfine` in CI
- Lazy initialization for heavy subsystems (tracing-subscriber, tree-sitter grammars) — defer setup until first use

### PR-002: Git Operations

- Single `git diff --cached` call, parsed per-file (not N+1 calls)
- Async process spawning (no blocking the tokio event loop)
- Benchmark: 100 staged files processed in < 2s

### PR-003: Tree-sitter Parsing

- Parallel parsing via rayon (one `Parser` instance per file per thread — `Parser` is not `Send`/`Sync`)
- File size limit: skip tree-sitter for files > 100KB
- Cancellation support via `parser.set_cancellation_flag()`
- Lazy language grammar loading (don't load Python grammar if no Python files staged)
- **Language detection**: Use file extension as primary signal, shebang line (`#!/usr/bin/env python3`) as fallback for extensionless scripts. Gracefully skip files with unrecognized languages (no error, just omit symbols from prompt).

### PR-004: LLM Generation

- Streaming output (tokens displayed as they arrive)
- Configurable timeout (default 300s)
- Cancellation via Ctrl+C with clean resource cleanup
- Connection health check before generation attempt

### PR-005: Memory

- Token budget system prevents unbounded growth (max_context_chars configurable, default 24K)
- **Budget unit**: Characters (fast estimation, no tokenizer dependency). Internal budget is measured in chars, not tokens; the `max_context_chars` config name reflects this.
- **Truncation priority** (highest to lowest): symbols > file list > diff hunks. When the budget is exceeded, diff hunks are truncated first, then file list entries, then symbols. Symbols are the most information-dense context and are preserved as long as possible.
- Tree-sitter parse trees dropped after symbol extraction
- Streaming line buffer bounded (max 1MB)
- Reduce tokio features to minimize binary bloat

### PR-006: Binary Size

- Feature-gated language support
- `[profile.release]` with `lto = true`, `strip = true`, `codegen-units = 1`
- Target: < 15MB release binary with default features

### PR-007: Cancellation Contract

- **Guarantee**: Cancellation via Ctrl+C (or `CancellationToken`) at any point in the pipeline results in **no partial commit** and **no leftover temp files**.
- LLM streaming cancellation drops the response and returns to prompt (or exits in non-interactive mode).
- Git commit is only called after the user confirms the complete message. No intermediate state is written to the repository.
- Temp files (if any) are cleaned up via RAII (`Drop` impl or `tempfile` crate auto-cleanup).

## 7. UX Requirements

### UX-001: Error Messages

Every error must include:

- **What** went wrong (clear, non-technical language)
- **Why** it might have happened (context)
- **How** to fix it (actionable suggestion)

Examples:

```bash
x Cannot connect to Ollama at http://localhost:11434

  help: Is Ollama running? Start it with:
        ollama serve
```

```bash
x No staged changes found

  help: Stage your changes first:
        git add <files>
```

### UX-002: Terminal Output

- Respect `NO_COLOR` environment variable
- Spinner during analysis and generation phases (suppressed in non-TTY)
- Streaming LLM output displayed in real-time
- Clear phase indicators: "Analyzing -> Generating -> Done"
- ASCII fallback for terminals that don't support Unicode well

### UX-003: Non-Interactive Mode

- `--yes` flag auto-confirms
- Non-TTY detection for git hooks and CI
- All output goes to stderr except the commit message itself (for piping)
- Exit codes: 0 success, 1 error, 2 usage error, 130 interrupted

### UX-004: CLI Design

```bash
commitbee [OPTIONS]                    # Generate and commit (default)
commitbee --dry-run                    # Generate, print, don't commit
commitbee --yes                        # Generate and auto-commit
commitbee --generate N                 # Generate N options
commitbee --show-prompt                # Debug: show LLM prompt
commitbee --verbose / -v               # Verbose output
commitbee --no-split                   # Disable commit split suggestions
commitbee --no-scope                   # Disable scope in commit messages
commitbee --clipboard                  # Copy to clipboard

commitbee init                         # Create config file
commitbee config                       # Show configuration
commitbee config check                 # Validate configuration
commitbee config set-key <provider>    # Store API key in keychain
commitbee doctor                       # Check Ollama connectivity, model availability

commitbee hook install                 # Install git hook
commitbee hook uninstall               # Remove git hook
commitbee hook status                  # Check hook status

commitbee completions <shell>          # Generate shell completions
commitbee eval                         # Run evaluation harness (dev tool)
```

### UX-005: First-Run Experience

- If no config exists and Ollama is detected, work with zero configuration
- If Ollama not found and no cloud provider configured, show helpful setup guidance
- `commitbee init` creates a well-commented config file with all options documented

### UX-006: Output Format Contracts

Exact output behavior for key flags:

- **`--dry-run`**: Prints the commit message to **stdout** (one line: `type(scope): description`). All other output (spinners, diagnostics, phase indicators) goes to stderr. Exit code 0.
- **`--generate N`**: In TTY mode, displays N numbered options and a `dialoguer` selection prompt on stderr; prints the selected message to stdout. In non-TTY mode, prints all N messages to stdout separated by a blank line. `--yes` selects the first option.
- **`--show-prompt`**: Prints the full LLM prompt to stderr (system prompt + user prompt). API keys and secret patterns are **redacted** (replaced with `[REDACTED]`). Does not call the LLM. Exit code 0.
- **Default (interactive)**: Displays the generated message and a confirm/edit/cancel prompt on stderr. On confirm, commits and prints the commit hash to stdout.

## 8. Testing Requirements

### TR-001: Unit Tests

| Module                      | Technique                   | Coverage Target                                     |
| --------------------------- | --------------------------- | --------------------------------------------------- |
| `CommitSanitizer`           | Snapshot (insta) + proptest | All code paths + never-panic guarantee              |
| `DiffHunk::parse_from_diff` | Snapshot                    | Standard diffs, renames, binary, empty              |
| `safety::scan_for_secrets`  | Unit + proptest             | Each pattern + false positive tests                 |
| `ContextBuilder`            | Snapshot                    | Budget calculation, type inference, scope inference |
| `FileCategory::from_path`   | Unit                        | All categories, edge cases                          |
| `CommitType`                | Unit                        | Verify `ALL` matches enum variants                  |

#### Golden Semantic Fixtures

A dedicated set of golden test fixtures in `tests/fixtures/golden/` that prove the semantic analysis advantage. Each fixture contains a before/after file pair, the expected diff, and the expected symbol extraction output. Scenarios include:

- **Moved function**: Function relocated within a file (diff shows delete + add, symbols show single move)
- **Signature change**: Function parameter or return type modified
- **Refactor extract**: Code extracted into a new function (symbols show new function + modified caller)
- **Rename symbol**: Variable or function renamed across multiple sites
- **Multi-file change**: Related changes spanning multiple files with shared symbol references

These fixtures serve as regression tests for the tree-sitter analysis pipeline and document the semantic advantage over raw diff approaches.

### TR-002: Integration Tests

| Scenario                          | Setup                              | Mock                                  |
| --------------------------------- | ---------------------------------- | ------------------------------------- |
| Normal commit flow                | tempfile git repo                  | wiremock Ollama                       |
| Empty staging area                | tempfile git repo                  | None                                  |
| Binary files mixed with text      | tempfile git repo                  | wiremock Ollama                       |
| Large diff (truncation)           | tempfile git repo                  | wiremock Ollama                       |
| Unicode file paths                | tempfile git repo                  | wiremock Ollama                       |
| LLM returns invalid JSON          | tempfile git repo                  | wiremock Ollama                       |
| LLM returns error mid-stream      | tempfile git repo                  | wiremock Ollama                       |
| Ollama not running                | None                               | No mock (real connection refused)     |
| Secret detected                   | tempfile git repo                  | None                                  |
| Non-TTY mode                      | tempfile git repo + piped stdin    | wiremock Ollama                       |

### TR-003: CLI Tests

- Snapshot tests with `insta-cmd` for all flag combinations
- `--dry-run` output format
- `--show-prompt` output format
- `--help` output
- Error message formatting
- Exit codes

### TR-004: Property-Based Tests

```rust
// Sanitizer never panics on any input
proptest! {
    #[test]
    fn sanitizer_never_panics(s in "\\PC*") {
        let _ = CommitSanitizer::sanitize(&s);
    }
}

// Secret scanner never panics on any input
proptest! {
    #[test]
    fn secret_scanner_never_panics(s in "\\PC*") {
        let _ = scan_for_secrets(&s);
    }
}
```

### TR-005: CI Pipeline

- `cargo check` (fast feedback)
- `cargo clippy -- -D warnings`
- `cargo test` (all tests)
- `cargo audit` (dependency vulnerabilities)
- `cargo deny check` (license compliance)
- Run on: push to `development`, all PRs
- Matrix: stable Rust + MSRV (1.94)
- **Edition 2024**: Rust edition 2024 requires MSRV 1.94; let chains (Rust 1.94) raise the effective MSRV to 1.94. CI matrix explicitly tests both stable and 1.94 to verify compatibility.

### TR-006: Evaluation Harness (`commitbee eval`)

A developer-facing command (`commitbee eval`) that runs the full pipeline against a set of fixture diffs and compares generated commit messages against expected style snapshots. Not shipped in release builds (feature-gated behind `dev` or `eval` feature flag).

- **Fixtures**: Stored in `tests/fixtures/eval/`, each containing a staged diff, optional config overrides, and an expected output snapshot.
- **Output**: Pass/fail report per fixture, with diff of expected vs. actual message.
- **Purpose**: Regression testing for prompt engineering changes — ensures prompt template updates don't degrade quality across the fixture set.

### TR-007: Fuzzing (Future Enhancement)

`cargo fuzz` targets for the diff parser, sanitizer, and secret scanner. Priority: P2 — implement after the unit test suite (TR-001) and property tests (TR-004) are stable. Fuzz targets should be added to `fuzz/` directory following standard `cargo-fuzz` conventions.

## 9. Distribution Requirements

### DR-001: cargo install

- `cargo install commitbee` works on all tier-1 platforms
- Published on crates.io with complete metadata

### DR-002: Prebuilt Binaries

- GitHub Releases via `cargo-dist`
- Platforms: macOS ARM64, macOS x86_64, Linux x86_64, Linux ARM64, Windows x86_64
- Shell installer: `curl -sSfL https://... | sh`
- Checksums and GitHub attestations

### DR-003: Homebrew

- Homebrew tap: `brew install sephyi/tap/commitbee`
- Generated automatically by `cargo-dist`

### DR-004: Shell Completions

- bash, zsh, fish, powershell
- Generated via `clap_complete`
- `commitbee completions <shell>` command
- Documented installation per shell in README

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
- Uses a JSON schema template with nullable fields and 2 micro few-shot examples (API replacement, style-only change) optimized for <4B parameter models
- **Concrete entity rule**: Subject must name at least one concrete entity (function, struct, variable, file) from the diff — integrated directly into the Subject rule line
- Negative examples (BAD/GOOD pairs): flags vague subjects ("update code and improve things") and multi-concern subjects ("refactor code for better performance and add validation") alongside positive examples
- Explicitly states what NOT to do (no conversational tone, no file-by-file listing, no business language)
- Anti-hallucination rules: "Never copy labels, field names, or evidence tags from the prompt into your output"
- API replacement rule: "If public APIs are both added and removed, this is an API replacement (refactor), not a new feature"
- Requests JSON output with explicit schema; includes breaking change guidance (only set when existing users or dependents must change their code, config, or scripts)
- Single shared constant (`pub(crate) SYSTEM_PROMPT` in `llm/mod.rs`) used by all providers; commit type list kept in sync with `CommitType::ALL` via compile-time test

### PE-002: User Prompt

- Includes: file list with change status, semantic symbols (functions/classes changed), truncated diff
- Symbols section with tri-state: "Added", "Removed", and "Modified (signature changed)" categories — modified public symbols contribute to breaking risk
- Suggested type and scope from heuristics (as hints, not requirements)
- **Evidence flags**: Natural language labels (not snake_case identifiers) to prevent small models from copying internal names. Questions like "Is this a mechanical/formatting change? yes/no" instead of `mechanical_transform: true`
- **Subject budget**: Computes remaining characters for subject after `type(scope): ` prefix, tells model the exact limit (e.g., "under 55 chars")
- **Primary change detection**: `PRIMARY_CHANGE:` line anchors subject to the most significant change, ranked: new public API > removed public API > largest file by lines changed
- **CONSTRAINTS section**: Dynamically generated rules based on evidence (e.g., "No bug-fix comments found — do not use type fix" when `has_bug_evidence=false`), includes metadata breaking constraints when detected
- **PUBLIC API REMOVED warning**: When public symbols are removed, a dedicated warning section lists them and instructs the model to describe removals in `breaking_change` field
- **Metadata breaking signals**: When MSRV, engines.node, or requires-python changes detected, a dedicated warning section lists them
- **Group rationale**: Per-group prompts include `GROUP_REASON:` explaining why files were grouped together
- **Focus instruction**: Groups with >5 files get explicit instruction to focus subject on the single most significant change
- Clear structure with headers for each section

### PE-003: Multi-Stage for Large Diffs

- When diff exceeds 50% of token budget: two-stage approach
- Stage 1: Per-file summary (fast model or heuristic)
- Stage 2: Commit message from summaries
- Fallback: single-stage with aggressive truncation (current approach)

### PE-004: Model-Specific Tuning

- Temperature: 0.0-0.3 (configurable)
- `num_predict` / `max_tokens`: 256 default (configurable)
- Stop sequences appropriate to model family
- System prompt complexity scaled to model size

### PE-005: Binary File Handling

- Binary files (images, compiled assets, archives) are **never** included as diff content in the prompt.
- Binary files **are** listed in the file list with their change status and size delta (e.g., `+ assets/logo.png (binary, +24KB)`).
- This provides the LLM enough context to mention binary changes without wasting budget on unreadable content.

### PE-006: JSON Parse Failure Recovery

- If the LLM returns invalid JSON, retry **once** with a repair prompt: "Your previous response was not valid JSON. Please respond with only valid JSON matching the schema."
- If the retry also fails, fall back to heuristic extraction: infer commit type from the diff header and file categories, extract the first coherent sentence as the commit message description.
- Never retry more than once (avoid infinite loops with models that consistently produce invalid output).

## 11. Phased Roadmap

### Phase 1: Shipped (v0.2.0)

**Goal**: Stability, correctness, rich providers, developer experience. All features below shipped in v0.2.0.

- FR-001: Fix UTF-8 panics ✅
- FR-002: Include symbols in prompt (with fallback ladder) ✅
- FR-003: Unit test suite (178 tests) ✅
- FR-004: Remove unused dependencies ✅
- FR-005: Fix dead code ✅
- FR-006: Reduce tokio features ✅
- FR-007: CommitType single source of truth ✅
- FR-010: miette diagnostics ✅
- FR-011: OpenAI-compatible provider (with streaming) ✅
- FR-012: Anthropic provider (with streaming) ✅
- FR-013: Ollama hardening (with streaming) ✅
- FR-014: Git hook integration (with edge case handling) ✅
- FR-015: Shell completions ✅
- FR-016: Multiple message generation (with `dialoguer`) ✅
- FR-017: figment configuration (with platform-specific paths) ✅
- FR-018: tracing logging ✅
- FR-019: Secure API key storage ✅ (feature-gated)
- FR-020: Async git operations ✅
- FR-021: Single-pass diff parsing ✅
- FR-022: Integration test suite ✅ (178 tests)
- FR-023: Commit splitting ✅
- FR-039: Config validation & doctor command ✅ (shipped in v0.2.0)
- TR-005: CI pipeline ✅

### Phase 2: Differentiation (v0.3.0)

**Goal**: Features that set commitbee apart from competitors.

- FR-040: Conventional Commits 1.0.0 spec anchoring ✅ (already implemented, enhanced with cross-project support)
- FR-041: Post-generation validation ✅ (already implemented)
- FR-034: Improved commit type heuristics ✅ (fully implemented — evidence flags, API replacement, mechanical detection, metadata breaking, symbol tri-state)
- FR-030: Custom prompt templates
- FR-031: Exclude files
- FR-032: Multi-language commit messages
- FR-033: Copy to clipboard
- FR-035: Rename detection
- FR-036: Tree-sitter query patterns
- FR-037: Expanded secret scanning
- FR-038: Progress indicators
- TR-006: Evaluation harness
- TR-007: Fuzzing targets

### Phase 3: Market Leadership (v0.4.0+)

**Goal**: Features that make commitbee the definitive tool in the category.

- FR-050: MCP server mode
- FR-051: Changelog generation
- FR-052: Multi-provider concurrent generation
- FR-053: Interactive regeneration with feedback
- FR-054: Monorepo support
- FR-055: Version bumping
- FR-056: GitHub Action
- FR-057: Additional language support (feature-gated)
- FR-058: Commit history style learning (experimental)

## 12. Success Metrics

| Metric                                | Target                              | How to Measure                                               |
| ------------------------------------- | ----------------------------------- | ------------------------------------------------------------ |
| Runtime panics                        | 0                                   | proptest + fuzzing, no `unwrap()` on user-facing paths       |
| Test coverage                         | > 80% on services/                  | `cargo tarpaulin`                                            |
| CI green rate                         | > 99%                               | GitHub Actions dashboard                                     |
| Cold startup time                     | < 200ms                             | `hyperfine` in CI                                            |
| Binary size (default features)        | < 15MB                              | CI artifact size tracking                                    |
| Commit message quality                | > 80% "good enough" on first try    | Manual evaluation on sample repos + `commitbee eval` harness |
| Secret leak rate                      | 0 (no secrets sent to cloud LLMs)   | Integration tests with known secret patterns                 |
| MSRV                                  | Rust 1.94 (edition 2024)            | CI matrix build (stable + 1.94)                              |

## 13. Non-Goals (Explicit Scope Exclusions)

- **GUI/TUI application** — CommitBee is CLI-first. Editor integration happens via MCP server mode, not a built-in UI.
- **General-purpose code review** — CommitBee generates commit messages. Code review is a different tool.
- **Git client replacement** — CommitBee wraps git for commit generation. It doesn't replace `git add`, `git push`, etc.
- **WASM plugin system** — Over-engineering for current scale. Configuration-driven extensibility first.
- **Non-git VCS** — Jujutsu/Mercurial support is not a priority. Git covers > 95% of the market.
- **Shell snippet detection** — Commit messages are never executed by git; shell injection via commit message content is not a real attack vector. Standard sanitization (FR-001, FR-007) is sufficient.

## Appendix A: Competitive Feature Matrix

| Feature               | commitbee  | opencommit | aicommits | aicommit2 | rusty-commit | cocogitto |
| --------------------- | ---------- | ---------- | --------- | --------- | ------------ | --------- |
| **Tree-sitter AST**   | **Yes**    | No         | No        | No        | No           | No        |
| **Commit splitting**  | **Yes**    | No         | No        | No        | No           | No        |
| **Secret scanning**   | **Yes**    | No         | No        | No        | No           | No        |
| **Token budget**      | **Yes**    | No         | No        | No        | No           | N/A       |
| **Streaming**         | **Yes**    | No         | No        | No        | No           | N/A       |
| **Local LLM**         | Yes        | Yes        | Yes       | Yes       | Yes          | N/A       |
| **OpenAI**            | **Yes**    | Yes        | Yes       | Yes       | Yes          | N/A       |
| **Anthropic**         | **Yes**    | Yes        | No        | Yes       | Yes          | N/A       |
| **Git hooks**         | **Yes**    | Yes        | Yes       | No        | Yes          | Yes       |
| **Multi-generate**    | **Yes**    | Yes        | Yes       | No        | No           | No        |
| **Shell completions** | **Yes**    | No         | No        | No        | No           | Yes       |
| **MCP server**        | Planned    | No         | No        | No        | Yes          | No        |
| **Changelog**         | Future     | No         | No        | No        | No           | Yes       |
| **Version bumping**   | Future     | No         | No        | No        | No           | Yes       |
| **Monorepo**          | Future     | No         | No        | No        | No           | Yes       |

## Appendix B: Research Sources

This PRD was informed by:

1. **Codebase analysis** — Line-by-line review of all 2,422 lines across 17 source files
2. **Competitor analysis** — 30+ tools across TypeScript, Rust, Python, Go reviewed
3. **Best practices research** — State-of-the-art Rust CLI patterns, LLM prompt engineering, tree-sitter techniques, security practices, testing strategies, distribution approaches
