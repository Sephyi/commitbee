<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
-->

# CommitBee

AI-powered commit message generator using tree-sitter semantic analysis and local LLMs.

## Quick Start

```bash
cargo build --release
./target/release/commitbee
```

## Architecture

- **Hybrid Git**: gix for repo discovery, git CLI for diffs (documented choice)
- **Tree-sitter**: Full file parsing with hunk mapping (not just +/- lines)
- **LLM**: Ollama primary (qwen3:4b), OpenAI/Anthropic secondary
- **Streaming**: Line-buffered JSON parsing with CancellationToken

## Key Design Decisions

1. **Full file parsing** - Parse staged/HEAD blobs, map diff hunks to symbol spans
2. **Token budget** - 24K char limit (~6K tokens), prioritizes diff over symbols
3. **TTY detection** - Safe for git hooks (graceful non-interactive fallback)
4. **Commit sanitizer** - Validates LLM output, supports JSON + plain text
5. **Structured JSON output** - Prompt requests JSON for reliable parsing
6. **System prompt** - Ollama API gets a dedicated system prompt to guide smaller models
7. **Simplified user prompt** - Concise format optimized for <4B parameter models
8. **Commit splitting** - Detects multi-concern changes, suggests splitting into separate commits
9. **Body line wrapping** - Sanitizer wraps body text at 72 characters

## Commands

```bash
commitbee                    # Generate commit message (interactive)
commitbee --dry-run          # Print message only, don't commit
commitbee --yes              # Auto-confirm and commit
commitbee -n 3               # Generate 3 candidates, pick interactively
commitbee --verbose          # Show symbol extraction details
commitbee --show-prompt      # Debug: show the LLM prompt
commitbee --no-split         # Disable commit split suggestions
commitbee --no-scope         # Disable scope in commit messages
commitbee init               # Create config file
commitbee config             # Show current configuration
commitbee doctor             # Check configuration and connectivity
commitbee completions bash   # Generate shell completions
commitbee hook install       # Install prepare-commit-msg hook
commitbee hook uninstall     # Remove prepare-commit-msg hook
commitbee hook status        # Check if hook is installed
```

## Config

Location: platform-dependent (use `commitbee init` to create, `commitbee doctor` to show path)

```toml
provider = "ollama"
model = "qwen3:4b"
ollama_host = "http://localhost:11434"
max_diff_lines = 500
max_file_lines = 100
max_context_chars = 24000
```

## Environment Variables

- `COMMITBEE_PROVIDER` - ollama, openai, anthropic
- `COMMITBEE_MODEL` - Model name
- `COMMITBEE_OLLAMA_HOST` - Ollama server URL
- `COMMITBEE_API_KEY` - API key for cloud providers

## Supported Languages (tree-sitter)

Rust, TypeScript, JavaScript, Python, Go

## File Structure

```bash
src/
├── main.rs              # Entry point
├── lib.rs               # Library exports
├── app.rs               # Application orchestrator
├── cli.rs               # CLI arguments (clap)
├── config.rs            # Configuration (figment layered)
├── error.rs             # Error types (thiserror + miette)
├── domain/
│   ├── mod.rs
│   ├── change.rs        # FileChange, StagedChanges, ChangeStatus
│   ├── symbol.rs        # CodeSymbol, SymbolKind
│   ├── context.rs       # PromptContext
│   └── commit.rs        # CommitType
└── services/
    ├── mod.rs
    ├── git.rs           # GitService (gix + git CLI)
    ├── analyzer.rs      # AnalyzerService (tree-sitter)
    ├── context.rs       # ContextBuilder (token budget)
    ├── safety.rs        # Secret scanning, conflict detection
    ├── sanitizer.rs     # CommitSanitizer (JSON + plain text)
    ├── splitter.rs      # CommitSplitter (multi-commit detection)
    └── llm/
        ├── mod.rs       # LlmProvider trait + enum dispatch
        ├── ollama.rs    # OllamaProvider (streaming NDJSON)
        ├── openai.rs    # OpenAiProvider (SSE streaming)
        └── anthropic.rs # AnthropicProvider (SSE streaming)
```

## References

- **PRD & Roadmap**: `PRD.md`
- **v0.3.0 enhancement plan**: `.claude/plans/PLAN_V030_ENHANCEMENTS.md`
- **Implementation plan (v1, outdated)**: `.claude/plans/PLAN_COMMITBEE_V1.md` — superseded by PRD v2.1

## Development Notes

### Toolchain

- Rust edition 2024, MSRV 1.85
- License: PolyForm-Noncommercial-1.0.0 (REUSE compliant)
- Dev deps: `tempfile`, `assert_cmd`, `predicates`, `wiremock`, `insta`, `proptest`

### REUSE / SPDX Headers

- All files use `reuse annotate` format: blank comment separator between SPDX lines
- `reuse lint` — verify compliance
- `reuse annotate --copyright "Sephyi <me@sephy.io>" --license PolyForm-Noncommercial-1.0.0 --year 2026 <file>` — add header
- REUSE.toml `[[annotations]]` — for files that can't have inline headers (Cargo.lock, tests/snapshots/**)

### Running Tests

```bash
cargo test                    # All tests (118 tests)
cargo test --test sanitizer   # CommitSanitizer tests
cargo test --test safety      # Safety module tests
cargo test --test context     # ContextBuilder tests
cargo test --test commit_type # CommitType tests
cargo test --test integration # LLM provider integration tests (wiremock)
cargo test -- --nocapture     # Show println output
```

**Important:** `cargo test sanitizer` matches test *names* across all binaries. Use `cargo test --test <name>` to select a specific integration test file.

### Building

```bash
cargo build --release         # Optimized binary
cargo check                   # Fast syntax check
cargo clippy --all-targets -- -D warnings  # Lint (CI requires zero warnings)
cargo fmt                     # Format code
```

### CI Verification Gate

Before pushing, run the full CI check locally:

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets
```

### Testing Manually

```bash
# Stage a change
git add some-file.rs

# Preview commit message
./target/release/commitbee --dry-run

# With verbose output
./target/release/commitbee --dry-run --verbose

# Debug the prompt
./target/release/commitbee --dry-run --show-prompt

# Auto-commit
./target/release/commitbee --yes
```

### Gotchas

- `gix` API: use `repo.workdir()` not `repo.work_dir()` (deprecated)
- `CommitType::parse()` not `from_str()` — avoids clippy `should_implement_trait` warning
- Enum variants used only via `CommitType::ALL` const need `#[allow(dead_code)]`
- Parallel subagents running `cargo fmt` may create unstaged changes — commit formatting separately
- Secret pattern `sk-[a-zA-Z0-9]{48}` requires exactly 48 chars after `sk-` in test data
- `tokio::process::Command` output needs explicit `std::process::Output` type annotation when using `.ok()?`
- Tree-sitter is CPU-bound/sync — pre-fetch file content into HashMaps async, then pass as sync closures
- `#[cfg(feature = "secure-storage")]` gates both the error variant and CLI commands for keyring

### Known Issues

- **No streaming during split generation**: When commit splitting generates per-group messages, LLM output is not streamed to the terminal (tokens are consumed silently). Single-commit generation streams normally. Low priority — split generation is fast since each sub-prompt is smaller.
- **Thinking model output**: Models with thinking enabled (e.g. `qwen3:4b` default) prepend `<think>...</think>` blocks that can break sanitizer parsing. The `hopephoto/Qwen3-4B-Instruct-2507_q8` variant does not exhibit this. Fix needed: strip thinking blocks in sanitizer pre-processing and/or pass `think: false` in Ollama API options.

### Markdown Conventions

- No `---` horizontal rules before `#` or `##` headers (they provide their own visual separation)
- Tables must use properly aligned columns with `| --- |` separator rows
