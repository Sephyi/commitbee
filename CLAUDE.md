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
2. **Token budget** - 24K char limit, prioritizes diff over symbols
3. **TTY detection** - Safe for git hooks (graceful non-interactive fallback)
4. **Commit sanitizer** - Validates LLM output, supports JSON + plain text

## Commands

```bash
commitbee              # Generate commit message
commitbee --dry-run    # Print message only
commitbee --yes        # Auto-confirm
commitbee init         # Create config
commitbee config       # Show config
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

## Implementation Progress

### Phase 1: Foundation

- [x] Cargo.toml with dependencies
- [x] Error types (src/error.rs)
- [x] Domain models (src/domain/*)
- [x] CLI args (src/cli.rs)

### Phase 2: Services

- [x] Config service (src/config.rs)
- [x] Git service (src/services/git.rs)
- [x] Analyzer service (src/services/analyzer.rs)
- [x] Context builder (src/services/context.rs)

### Phase 3: LLM Integration

- [x] Provider trait (src/services/llm/mod.rs)
- [x] Ollama provider (src/services/llm/ollama.rs)
- [x] Commit sanitizer (src/services/sanitizer.rs)

### Phase 4: Application

- [x] Safety service (src/services/safety.rs)
- [x] App orchestrator (src/app.rs)
- [x] Main entry (src/main.rs)

### Phase 5: Polish

- [ ] Tests
- [ ] OpenAI/Anthropic providers
- [ ] Documentation

---

## Build Status

**Last build**: 2026-02-02 - Release build successful
