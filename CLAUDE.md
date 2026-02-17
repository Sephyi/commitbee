<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: GPL-3.0-only
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

## Commands

```bash
commitbee              # Generate commit message (interactive)
commitbee --dry-run    # Print message only, don't commit
commitbee --yes        # Auto-confirm and commit
commitbee --verbose    # Show symbol extraction details
commitbee --show-prompt # Debug: show the LLM prompt
commitbee init         # Create config file
commitbee config       # Show current configuration
```

## Config

Location: `~/.config/commitbee/config.toml`

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

```
src/
├── main.rs              # Entry point
├── lib.rs               # Library exports
├── app.rs               # Application orchestrator
├── cli.rs               # CLI arguments (clap)
├── config.rs            # Configuration (XDG + ENV)
├── error.rs             # Error types (thiserror)
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
    └── llm/
        ├── mod.rs       # LlmProvider trait
        └── ollama.rs    # OllamaProvider (streaming)
```

## References

- **PRD & Roadmap**: `PRD.md`
- **Implementation plan (v1, outdated)**: `.claude/plans/PLAN_COMMITBEE_V1.md` — superseded by PRD v2.1

## Development Notes

### Toolchain

- Rust edition 2024, MSRV 1.85
- License: GPL-3.0-only (REUSE compliant)
- Dev deps: `tempfile`, `assert_cmd`, `predicates`, `wiremock`

### REUSE / SPDX Headers

- All files use `reuse annotate` format: blank comment separator between SPDX lines
- `reuse lint` — verify compliance
- `reuse annotate --copyright "Sephyi <me@sephy.io>" --license GPL-3.0-only --year 2026 <file>` — add header
- REUSE.toml `[[annotations]]` — only for files that can't have inline headers (e.g., Cargo.lock)

### Running Tests (when implemented)

```bash
cargo test                    # All tests
cargo test sanitizer          # Specific module
cargo test -- --nocapture     # Show println output
```

### Building

```bash
cargo build --release         # Optimized binary
cargo check                   # Fast syntax check
cargo clippy                  # Lint checks
cargo fmt                     # Format code
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
