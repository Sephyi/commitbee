<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
-->

# CommitBee Documentation

> The commit message generator that actually understands your code.

This guide covers everything CommitBee does, how it does it, and how to get the most out of it. Whether you're setting it up for the first time or curious about the internals, this is the place.

## Table of Contents

- [Getting Started](#-getting-started)
- [How It Works](#-how-it-works)
- [Configuration](#-configuration)
- [Commands & Flags](#-commands--flags)
- [LLM Providers](#-llm-providers)
- [Commit Splitting](#-commit-splitting)
- [The Validation Pipeline](#-the-validation-pipeline)
- [Security & Safety](#-security--safety)
- [Git Hook Integration](#-git-hook-integration)
- [Supported Languages](#-supported-languages)
- [Troubleshooting](#-troubleshooting)
- [Architecture Deep Dive](#-architecture-deep-dive)

## 🚀 Getting Started

### Install

```bash
cargo install commitbee
```

Or build from source:

```bash
git clone https://github.com/sephyi/commitbee.git
cd commitbee
cargo build --release
# Binary at ./target/release/commitbee
```

### Requirements

- **Rust 1.94+** (edition 2024)
- **Ollama** running locally — [ollama.ai](https://ollama.ai)
- A model pulled: `ollama pull qwen3.5:4b`

### First Run

```bash
# Stage something
git add src/my_change.rs

# Generate a commit message
commitbee
```

That's it. Zero configuration needed if Ollama is running with `qwen3.5:4b`.

CommitBee will analyze your staged changes, extract semantic information via tree-sitter, send a structured prompt to the LLM, validate the output, and present you with a commit message to approve.

### Quick Config

Want to customize things? Create a config file:

```bash
commitbee init
```

This creates a config at your platform's standard location (run `commitbee doctor` to see where). Edit it to change the model, provider, or formatting preferences.

## 🧠 How It Works

Most commit message generators dump `git diff` output into an LLM and hope for the best. CommitBee takes a fundamentally different approach.

### The Pipeline

```txt
Stage Changes → Git Service → Tree-sitter → Splitter → Context Builder → LLM → Validator → Sanitizer
```

Here's what each step actually does:

**1. Git Service** reads your staged changes using `gix` for repo discovery and the git CLI for diffs. Paths are parsed with NUL-delimited output (`-z` flag) so filenames with spaces or special characters work correctly.

**2. Tree-sitter Analyzer** parses both the staged version and the HEAD version of every changed file — in parallel, using `rayon` across CPU cores. It maps diff hunks to symbol spans, so instead of just knowing "lines 42-58 changed", CommitBee knows "the `validate()` function in `sanitizer.rs` was modified". Symbols are tracked in three states: added, removed, or modified-signature.

**3. Commit Splitter** looks at your staged changes and decides whether they contain logically independent work. It uses diff-shape fingerprinting (what kind of changes — additions, deletions, modifications) combined with Jaccard similarity on content vocabulary to group files. If it finds multiple concerns, it offers to split them into separate commits.

**4. Context Builder** assembles a budget-aware prompt. It computes evidence flags from the code analysis (is this a mechanical change? are public APIs removed? is there bug-fix evidence?), calculates the character budget for the subject line, and packs as much useful context as possible within the token limit (~6K tokens by default).

**5. LLM Provider** streams the prompt to your chosen model (Ollama, OpenAI, or Anthropic) and collects the response token by token.

**6. Validator** checks the LLM's output against the evidence flags. If the model says "fix" but there's no bug-fix evidence in the code, or if the subject is too long, or if it used generic wording — the validator catches it and retries with targeted correction instructions. Up to 3 attempts.

**7. Sanitizer** does the final cleanup: extracts JSON from potentially noisy LLM output (thinking blocks, code fences, conversational preambles), validates the conventional commit format, wraps the body at 72 characters, and constructs the final commit message string.

### What Makes the Prompt Special

CommitBee doesn't just send a diff. The prompt includes:

- **File summary** with per-file line counts (`+additions -deletions`)
- **Suggested commit type** inferred from code analysis (not guessed)
- **Evidence flags** telling the LLM deterministic facts about the change
- **Symbol changes** — which functions, structs, and methods were added, removed, or modified
- **Primary change detection** — which file has the most significant changes
- **Constraints** — rules the LLM must follow based on evidence (e.g., "no bug-fix comments found, prefer refactor over fix")
- **Character budget** — exact number of chars available for the subject line
- **Group rationale** — when splitting, why these files are grouped together

All of this is computed before the LLM ever sees the diff. The model gets to focus on writing a good commit message rather than doing code analysis.

## ⚙️ Configuration

### Config File

CommitBee uses platform-standard config directories:

| Platform | Path |
| --- | --- |
| macOS | `~/Library/Application Support/commitbee/config.toml` |
| Linux | `~/.config/commitbee/config.toml` |
| Windows | `%APPDATA%\commitbee\config\config.toml` |

Run `commitbee doctor` to see the exact path on your system.

### Full Config Reference

```toml
# LLM provider: ollama, openai, anthropic
provider = "ollama"

# Model name (for Ollama, use `ollama list` to see available)
model = "qwen3.5:4b"

# Ollama server URL
ollama_host = "http://localhost:11434"

# API key for cloud providers (OpenAI, Anthropic)
# Better: use COMMITBEE_API_KEY env var or `commitbee set-key`
# api_key = "sk-..."

# Maximum lines of diff to include in prompt (10-10000)
max_diff_lines = 500

# Maximum lines per file in diff (10-1000)
max_file_lines = 100

# Maximum context characters for LLM prompt (~4 chars per token)
# Default 24000 is safe for 8K context models
# Increase for larger models (e.g., 48000 for 16K context)
max_context_chars = 24000

# Request timeout in seconds (1-3600)
timeout_secs = 300

# LLM temperature (0.0-2.0). Lower = more deterministic
temperature = 0.3

# Maximum tokens to generate (default 256)
# Increase to 8192+ if using thinking models with think = true
num_predict = 256

# Enable thinking/reasoning for Ollama models (default: false)
# When enabled, models like qwen3 will reason before responding.
# Requires higher num_predict (8192+) to accommodate thinking tokens.
think = false

# Commit message format options
[format]
# Include body/description in commit message
include_body = true

# Include scope in commit type, e.g., feat(scope): subject
include_scope = true

# Enforce lowercase first character of subject
lowercase_subject = true
```

### Config Priority

Configuration is layered (highest priority wins):

1. **CLI flags** — `--provider`, `--model`, `--no-scope`
2. **Environment variables** — `COMMITBEE_PROVIDER`, `COMMITBEE_MODEL`, etc.
3. **User config** — `config.toml` at the platform path
4. **Project config** — `.commitbee.toml` in the repository root
5. **Defaults** — built-in sensible defaults

This means you can set global preferences in your config file and override per-project with `.commitbee.toml` or per-invocation with env vars or flags.

### Environment Variables

| Variable | Description |
| --- | --- |
| `COMMITBEE_PROVIDER` | LLM provider (`ollama`, `openai`, `anthropic`) |
| `COMMITBEE_MODEL` | Model name |
| `COMMITBEE_OLLAMA_HOST` | Ollama server URL |
| `COMMITBEE_API_KEY` | API key for cloud providers |
| `COMMITBEE_LOG` | Log level filter (e.g., `debug`, `commitbee=debug`) |

Nested config keys use `__` as separator: `COMMITBEE_FORMAT__INCLUDE_BODY=false`.

## 🎯 Commands & Flags

### Main Usage

```bash
commitbee [FLAGS] [COMMAND]
```

When run without a command, CommitBee generates a commit message for your staged changes.

### Flags

| Flag | Short | Description |
| --- | --- | --- |
| `--dry-run` | | Print message only, don't commit |
| `--yes` | `-y` | Auto-confirm and commit without prompting |
| `--generate N` | `-n N` | Generate N candidates (1-5), pick interactively |
| `--no-split` | | Disable commit split suggestions |
| `--no-scope` | | Disable scope in commit messages |
| `--allow-secrets` | | Allow committing with detected secrets (Ollama only) |
| `--show-prompt` | | Display the full prompt sent to the LLM |
| `--verbose` | `-v` | Show symbol extraction details |
| `--provider` | `-p` | Override LLM provider |
| `--model` | `-m` | Override model name |

### Commands

| Command | Description |
| --- | --- |
| `init` | Create a config file at the platform path |
| `config` | Show current configuration values |
| `doctor` | Check configuration, connectivity, and model availability |
| `completions <shell>` | Generate shell completions (bash, zsh, fish, powershell) |
| `hook install` | Install `prepare-commit-msg` git hook |
| `hook uninstall` | Remove the git hook |
| `hook status` | Check if the hook is installed |

### Usage Patterns

```bash
# The basics
commitbee                        # Interactive: generate, review, commit
commitbee --dry-run              # Preview without committing
commitbee --yes                  # Non-interactive: generate and commit

# Debugging
commitbee --show-prompt          # See exactly what the LLM receives
commitbee --verbose              # See tree-sitter symbol extraction
COMMITBEE_LOG=debug commitbee    # Full debug logging

# Multiple candidates
commitbee -n 3                   # Generate 3 options, pick the best

# Scripting / CI
commitbee --yes --dry-run        # Generate message, print to stdout, exit
commitbee --no-split --yes       # Skip split suggestion, auto-commit
```

## 🤖 LLM Providers

CommitBee supports three providers. All use streaming for responsive output.

### Ollama (default, local)

The recommended setup. Your code never leaves your machine.

```toml
provider = "ollama"
model = "qwen3.5:4b"
ollama_host = "http://localhost:11434"
```

**Recommended models:**

| Model | Size | Notes |
| --- | --- | --- |
| `qwen3.5:4b` | 3.4 GB | Default. Fast, clean JSON output |
| `llama3:8b` | 4.7 GB | Good quality, slower |
| `codellama:7b` | 3.8 GB | Code-focused alternative |

**Thinking mode**: Some models (like `qwen3:4b`) have built-in reasoning that produces `<think>` blocks before their response. CommitBee can handle these — set `think = true` in your config and bump `num_predict` to `8192` or higher to give the model room for both thinking and output tokens. The default model `qwen3.5:4b` doesn't need this.

### OpenAI

```toml
provider = "openai"
model = "gpt-4o-mini"
api_key = "sk-..."
```

Or use environment variables:

```bash
export COMMITBEE_PROVIDER=openai
export COMMITBEE_MODEL=gpt-4o-mini
export OPENAI_API_KEY=sk-...
```

Works with any OpenAI-compatible API. Set `openai_base_url` for custom endpoints:

```toml
openai_base_url = "https://api.together.xyz/v1"
```

### Anthropic

```toml
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key = "sk-ant-..."
```

Or:

```bash
export COMMITBEE_PROVIDER=anthropic
export ANTHROPIC_API_KEY=sk-ant-...
```

### Secure Key Storage

If built with the `secure-storage` feature, CommitBee can store API keys in your OS keychain:

```bash
cargo install commitbee --features secure-storage
commitbee set-key openai      # Prompts for key, stores in keychain
commitbee set-key anthropic   # Same for Anthropic
commitbee get-key openai      # Check if key exists
```

Key lookup order: config file → environment variable → keychain.

## ✂️ Commit Splitting

One of CommitBee's standout features. When your staged changes contain logically independent work, CommitBee detects this and offers to create separate commits.

### How It Works

The splitter doesn't just look at directory structure. It uses two signals:

**Diff-shape fingerprinting** — Each file gets a "shape" based on its change pattern (ratio of additions to deletions, whether it's a new file, etc.). Files with similar shapes are more likely related.

**Jaccard similarity on content vocabulary** — The actual words in the diff are compared. If two files share similar vocabulary (same variable names, function names, imports), they're probably part of the same logical change.

Files are then grouped by combining these signals with category separation (tests stay with their source files, docs are separated from code, config files are grouped together).

### Example

```txt
⚡ Commit split suggested — 2 logical change groups detected:

  Group 1: feat(llm)  [2 files]
    [M] src/services/llm/anthropic.rs (+20 -5)
    [M] src/services/llm/openai.rs (+8 -3)

  Group 2: fix(sanitizer)  [1 file]
    [M] src/services/sanitizer.rs (+3 -1)

Split into separate commits? (Y/n)
```

If you accept, CommitBee will:
1. Generate a commit message for each group using a group-specific prompt
2. Show you all proposed commits for review
3. Execute them sequentially (unstage all → stage group files → commit → repeat)

### Limitations

- Requires an interactive terminal (no split in `--yes` mode or git hooks)
- Won't split if any staged files also have unstaged changes (safety check)
- Disable with `--no-split` if you know your change is intentionally combined

## 🔍 The Validation Pipeline

CommitBee doesn't blindly trust LLM output. Every generated message goes through a multi-stage validation pipeline.

### Stage 1: Evidence-Based Validation

Before the LLM generates anything, CommitBee computes five deterministic signals from your code:

| Signal | What It Detects |
| --- | --- |
| `is_mechanical` | Formatting-only changes (whitespace, import reordering) |
| `has_bug_evidence` | Bug-fix comments in the diff (`fix`, `bug`, `patch`) |
| `public_api_removed_count` | Removed public functions, structs, or traits |
| `has_new_public_api` | New public symbols added |
| `is_dependency_only` | All changes in dependency/config files |

After the LLM responds, the **CommitValidator** checks the output against these signals with 7 rules:

1. **Fix requires evidence** — `fix` type needs bug-fix comments in the diff, otherwise it should be `refactor`
2. **Breaking change detection** — If public APIs were removed, `breaking_change` must be set
3. **Anti-hallucination** — `breaking_change` must not copy internal field names from the prompt
4. **Mechanical = style** — Formatting-only changes can't be `feat` or `fix`
5. **Dependencies = chore** — Dependency-only changes must use `chore` type
6. **Subject specificity** — Rejects generic subjects like "update code" or "improve things"
7. **Subject length** — Rejects subjects that would produce a first line exceeding 72 characters

### Stage 2: Multi-Pass Retry

If any rules are violated, CommitBee appends a `CORRECTIONS` section to the prompt explaining what went wrong and re-prompts the LLM. It then **re-validates** the retry output. If violations persist, it retries again — up to 3 total attempts.

This is more sophisticated than a simple retry. Each attempt gets the full list of remaining violations, so the LLM can address them all at once.

### Stage 3: Sanitization

The final output goes through the sanitizer, which handles the messy reality of LLM output:

- **Thinking block removal** — Strips `<think>...</think>` and `<thought>...</thought>` blocks (even unclosed ones)
- **Code fence extraction** — Finds JSON inside `` ```json ... ``` `` blocks
- **Preamble stripping** — Removes conversational text like "Here's the commit message:" before the actual content
- **JSON parsing** — Extracts structured commit data from the LLM's JSON response
- **Format validation** — Verifies the result is a valid conventional commit
- **Body wrapping** — Wraps body text at 72 characters, preserving paragraph breaks
- **First line enforcement** — Rejects messages where the first line exceeds 72 characters

If the sanitizer can't produce a valid commit message, you get a clear error explaining what went wrong — never a silently mangled message.

## 🔒 Security & Safety

### Secret Scanning

Before anything is sent to an LLM, CommitBee scans all staged content for:

| Pattern | Examples |
| --- | --- |
| AWS access keys | `AKIA...` |
| OpenAI API keys | `sk-...` (48 chars) |
| Anthropic API keys | `sk-ant-api...` |
| Generic API keys | `api_key = "..."`, `API_KEY=...` |
| Private keys | `-----BEGIN RSA PRIVATE KEY-----` |
| Connection strings | `postgres://user:pass@host/db` |

If secrets are found:

- **Ollama (local)**: Warning displayed, proceeds (data stays on your machine)
- **Cloud providers**: Hard error, commit blocked. Use `--allow-secrets` to override (Ollama only)

Scanning only checks added lines — removed lines are ignored (they're already in git history).

### Merge Conflict Detection

CommitBee checks for unresolved merge conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) in staged changes. If found, the commit is blocked with an actionable error.

The conflict checker is smart about false positives:
- Ignores conflict markers in test files and documentation
- Ignores diff headers (lines starting with `---` or `+++`)
- Uses component-based path matching to avoid false positives from CommitBee's own source code

### Data Privacy

With the default Ollama provider, **no data ever leaves your machine**. The entire pipeline runs locally. Cloud providers (OpenAI, Anthropic) send the prompt over HTTPS — which includes your diff and symbol information. Choose your provider accordingly.

## 🪝 Git Hook Integration

CommitBee can run automatically when you `git commit`.

### Install the Hook

```bash
commitbee hook install
```

This creates a `prepare-commit-msg` hook that generates a commit message using CommitBee whenever you run `git commit` without a `-m` flag.

The hook:

- Skips merge, squash, amend, and message-provided commits
- Silently does nothing if `commitbee` isn't on your PATH
- Writes the generated message to the commit message file
- Runs in `--yes --dry-run` mode (non-interactive)

### Manage the Hook

```bash
commitbee hook status      # Check if installed
commitbee hook uninstall   # Remove (restores any backed-up previous hook)
```

If you already had a `prepare-commit-msg` hook, CommitBee backs it up as `prepare-commit-msg.commitbee-backup` and restores it on uninstall.

### TTY Safety

CommitBee detects whether it's running in an interactive terminal. In non-interactive contexts (git hooks, CI pipelines, piped output), it:

- Skips interactive prompts (candidate selection, split confirmation)
- Never blocks waiting for user input
- Prints messages to stdout for piping

## 🌳 Supported Languages

CommitBee uses tree-sitter to parse source files and extract semantic symbols. Currently supported:

| Language | Parser | What It Extracts |
| --- | --- | --- |
| Rust | `tree-sitter-rust` | Functions, structs, enums, impls, traits, methods |
| TypeScript | `tree-sitter-typescript` | Functions, classes, interfaces, methods, types |
| JavaScript | `tree-sitter-javascript` | Functions, classes, methods, arrow functions |
| Python | `tree-sitter-python` | Functions, classes, methods, decorators |
| Go | `tree-sitter-go` | Functions, types, methods, interfaces |

**Files in unsupported languages still work** — they're included in the diff context, they just don't get semantic symbol extraction. The commit message will still be based on the actual diff content; it just won't know which specific functions or types changed.

### Symbol Tracking

For supported languages, symbols are tracked in three states:

- **Added** `[+]` — New function, struct, class, etc.
- **Removed** `[-]` — Deleted symbol
- **Modified (signature changed)** `[~]` — Symbol exists in both versions but its signature changed

This information appears in the prompt as a `SYMBOLS CHANGED` section, giving the LLM precise knowledge of what was structurally modified.

## 🔧 Troubleshooting

### `commitbee doctor`

Your first stop for diagnosing issues. It checks:

- Config file location and existence
- Provider connectivity (can CommitBee reach Ollama/OpenAI/Anthropic?)
- Model availability (is the configured model actually pulled?)
- Git repository detection

### Common Issues

**"Empty response from LLM"**

The model ran out of tokens before producing output. Usually caused by thinking models consuming the token budget with `<think>` blocks.

Fix: Either switch to `qwen3.5:4b` (default, no thinking overhead) or increase `num_predict`:

```toml
num_predict = 8192
think = true
```

**"First line is X chars (max 72)"**

The LLM generated a subject line that's too long. CommitBee will retry up to 3 times with correction instructions. If it still fails, the error tells you exactly how long the line was. This is rare with the default model.

**"No staged changes found"**

You need to `git add` files before running CommitBee.

**"Cannot connect to Ollama"**

Ollama isn't running. Start it with `ollama serve` or check that the configured `ollama_host` is correct.

**"Model not found"**

The configured model isn't pulled. Run `ollama pull qwen3.5:4b` (or whichever model you've configured).

**"Potential secrets detected"**

CommitBee found something that looks like an API key or credential in your staged changes. If it's a false positive and you're using Ollama (local), use `--allow-secrets`. For cloud providers, this is a hard block — remove the secret from your staged changes.

### Debug Mode

For deep debugging, use `--show-prompt` to see the exact prompt sent to the LLM:

```bash
commitbee --dry-run --show-prompt
```

This prints the full prompt including the diff, evidence flags, constraints, symbol list, and character budget. Very useful for understanding why the LLM made a particular choice.

For internal tracing:

```bash
COMMITBEE_LOG=debug commitbee --dry-run
```

This shows config loading, symbol counts, sanitizer steps, validation violations, and retry attempts.

## 🏗️ Architecture Deep Dive

For contributors and the curious. This section covers the internal architecture.

### Crate Structure

```txt
src/
├── main.rs              # Entry point, tracing setup
├── lib.rs               # Library exports (for integration tests)
├── app.rs               # Application orchestrator (all the glue)
├── cli.rs               # CLI argument parsing (clap derive)
├── config.rs            # Configuration loading (figment layered)
├── error.rs             # Error types (thiserror + miette diagnostics)
├── domain/
│   ├── change.rs        # FileChange, StagedChanges, ChangeStatus
│   ├── symbol.rs        # CodeSymbol, SymbolKind (Added/Removed/Modified)
│   ├── context.rs       # PromptContext — assembles the LLM prompt
│   └── commit.rs        # CommitType enum (single source of truth)
└── services/
    ├── git.rs           # GitService — gix for discovery, git CLI for diffs
    ├── analyzer.rs      # AnalyzerService — tree-sitter parsing via rayon
    ├── context.rs       # ContextBuilder — evidence flags, token budget
    ├── safety.rs        # Secret scanning, conflict detection
    ├── sanitizer.rs     # CommitSanitizer + CommitValidator
    ├── splitter.rs      # CommitSplitter — diff-shape + Jaccard clustering
    └── llm/
        ├── mod.rs       # LlmBackend enum dispatch, SYSTEM_PROMPT
        ├── ollama.rs    # OllamaProvider — streaming NDJSON
        ├── openai.rs    # OpenAiProvider — SSE streaming
        └── anthropic.rs # AnthropicProvider — SSE streaming
```

### Key Design Decisions

**Hybrid Git** — `gix` (pure Rust) is used for fast repo discovery, but the git CLI is used for diffs and staging operations. This avoids the complexity of reimplementing diff parsing in pure Rust while keeping startup fast.

**Full File Parsing** — Tree-sitter parses the complete staged and HEAD versions of files, not just the diff hunks. Diff hunks are then mapped to symbol spans. This means CommitBee knows the full context of what changed, not just the changed lines.

**Enum Dispatch** — The LLM provider uses an enum (`LlmBackend`) rather than a trait object. This avoids `async-trait` overhead and the complexity of `dyn` dispatch for async methods.

**Streaming with Cancellation** — All providers support Ctrl+C cancellation via `tokio_util::CancellationToken`. The streaming display runs in a separate tokio task with `tokio::select!` for responsive cancellation.

**Token Budget** — The context builder tracks character usage (~4 chars per token) and truncates the diff if it exceeds the budget, prioritizing the most important files. The default 24K char budget (~6K tokens) is safe for 8K context models.

**Single Source of Truth for Types** — `CommitType::ALL` is a const array that defines all valid commit types. The system prompt's type list is verified at compile time (via a `#[test]`) to match this array exactly.

### Error Philosophy

Every error in CommitBee is:

- **Actionable** — Tells you what went wrong and how to fix it (via `miette` help messages)
- **Typed** — Uses `thiserror` for structured error variants, not string errors
- **Diagnostic** — Error codes like `commitbee::git::no_staged` for programmatic handling

No panics in user-facing code paths. The sanitizer and validator are tested with proptest to ensure they never panic on arbitrary input.

### Testing Strategy

CommitBee has 182 tests across multiple strategies:

| Strategy | What It Covers |
| --- | --- |
| Unit tests | Individual functions (sanitizer rules, type parsing, config defaults) |
| Snapshot tests (insta) | Output format stability |
| Property tests (proptest) | Never-panic guarantees for parsers |
| Integration tests (wiremock) | Full provider round-trips with mocked HTTP |
| Git fixture tests | Real git operations in temp directories |

Run them:

```bash
cargo test                    # All 182 tests
cargo test --test sanitizer   # Just sanitizer tests
cargo test --test integration # LLM provider mocks
COMMITBEE_LOG=debug cargo test -- --nocapture  # With logging
```
