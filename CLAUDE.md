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
```

## Environment Variables

- `COMMITBEE_PROVIDER` - ollama, openai, anthropic
- `COMMITBEE_MODEL` - Model name
- `COMMITBEE_OLLAMA_HOST` - Ollama server URL
- `COMMITBEE_API_KEY` - API key for cloud providers

---

## Project Status (v0.1.0)

**Last updated**: 2026-02-02
**Status**: Core functionality complete, prompt improved for small LLMs
**License**: GPL-3.0-only (REUSE compliant)

### Recent Improvements (2026-02-02)

1. **System prompt for Ollama** - Added dedicated system prompt to guide smaller models
2. **Simplified prompt template** - More concise format that smaller LLMs follow better
3. **REUSE compliance** - All source files have SPDX headers
4. **Dependency updates** - Updated to latest versions (reqwest 0.13, gix 0.78, tree-sitter 0.26, toml 0.9)

### Completed Features

| Component | File | Status |
|-----------|------|--------|
| CLI args | `src/cli.rs` | ✅ |
| Config (XDG + ENV) | `src/config.rs` | ✅ |
| Error types | `src/error.rs` | ✅ |
| Domain models | `src/domain/*` | ✅ |
| Git service | `src/services/git.rs` | ✅ |
| Tree-sitter analyzer | `src/services/analyzer.rs` | ✅ |
| Context builder | `src/services/context.rs` | ✅ |
| LLM provider trait | `src/services/llm/mod.rs` | ✅ |
| Ollama provider | `src/services/llm/ollama.rs` | ✅ |
| Secret scanning | `src/services/safety.rs` | ✅ |
| Commit sanitizer | `src/services/sanitizer.rs` | ✅ |
| App orchestrator | `src/app.rs` | ✅ |
| Main entry | `src/main.rs` | ✅ |

### Known Compiler Warnings (Intentional)

These are unused but reserved for future features:

- `old_path` field in `FileChange` - for rename tracking
- `signature` field in `CodeSymbol` - for richer symbol display
- `is_empty()` method on `StagedChanges`
- `CommitType` variants: Style, Perf, Ci, Revert
- `EmptyRepository` error variant

### Not Yet Implemented

| Item | Priority | Notes |
|------|----------|-------|
| **Unit tests** | High | Sanitizer, analyzer, context builder |
| **Integration tests** | High | CLI with mock Ollama (wiremock) |
| **OpenAI provider** | Medium | `src/services/llm/openai.rs` |
| **Anthropic provider** | Medium | `src/services/llm/anthropic.rs` |
| **Rename detection** | Low | Use `git diff --find-renames` |
| **`--redact-secrets`** | Low | Redact instead of blocking |
| **Config validation** | Low | Check model exists in Ollama |
| **Retry logic** | Low | Retry on transient Ollama failures |

### Edge Cases to Test

1. Very large diffs - token budget truncation
2. Binary files mixed with text - should be skipped
3. Ollama not running - clear error message?
4. Invalid JSON from LLM - fallback to plain text?
5. Unicode in file paths
6. Empty staged changes (already handled)
7. Merge conflicts in staged files

### Recommended Models

Available on user's system:

- `qwen3:4b` (default) - Good general performance
- `qwen2.5-coder:3b-instruct` - Better for code-heavy diffs
- `llama3.2:1b` - Faster but less capable

---

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

---

## Plan Reference

Full implementation plan with code samples: `.claude/plans/PLAN_COMMITBEE_V1.md`

This plan includes:

- Detailed architecture diagrams
- Code samples for all components
- Review feedback addressed (2 rounds)
- Dependency versions
- Test scaffolding

---

## Development Notes

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
