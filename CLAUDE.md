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
- **Parallelism**: rayon for CPU-bound tree-sitter parsing, tokio JoinSet for concurrent git content fetching
- **LLM**: Ollama primary (qwen3.5:4b), OpenAI/Anthropic secondary
- **Streaming**: Line-buffered JSON parsing with CancellationToken, 1 MB response cap (`MAX_RESPONSE_BYTES`)

## Key Design Decisions

1. **Full file parsing** - Parse staged/HEAD blobs, map diff hunks to symbol spans
2. **Token budget** - 24K char limit (~6K tokens), prioritizes diff over symbols
3. **TTY detection** - Safe for git hooks (graceful non-interactive fallback)
4. **Commit sanitizer** - Validates LLM output, supports JSON + plain text; emits `BREAKING CHANGE:` footer and `!` suffix for breaking changes (footer emitted regardless of `include_body` — it is machine-readable metadata)
5. **Structured JSON output** - Prompt requests JSON for reliable parsing; schema includes `breaking_change: Option<String>` field
6. **System prompt** - Single `pub(crate) const SYSTEM_PROMPT` in `llm/mod.rs`, shared by all providers; includes commit type list (synced with `CommitType::ALL`), project-agnostic breaking change threshold (only when existing users or dependents must change their code/config/scripts to stay compatible — not for new features, bug fixes, or internal refactors), and 72-char subject limit
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
commitbee --clipboard        # Copy message to clipboard (no commit)
commitbee --exclude "*.lock" # Exclude files matching glob pattern
commitbee --locale de        # Generate message in German (type/scope stay English)
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
model = "qwen3.5:4b"
ollama_host = "http://localhost:11434"
max_diff_lines = 500
max_file_lines = 100
max_context_chars = 24000
temperature = 0.3
num_predict = 256
timeout_secs = 300
think = false
rename_threshold = 70
learn_from_history = false
history_sample_size = 50
# locale = "de"
# exclude_patterns = ["*.lock", "**/*.generated.*"]
# system_prompt_path = "/path/to/system.txt"
# template_path = "/path/to/template.txt"

[format]
include_body = true
include_scope = true
lowercase_subject = true

[safety]
# custom_secret_patterns = ["CUSTOM_KEY_[a-zA-Z0-9]{32}"]
# disabled_secret_patterns = ["Generic Secret (unquoted)"]
```

## Environment Variables

- `COMMITBEE_PROVIDER` - ollama, openai, anthropic
- `COMMITBEE_MODEL` - Model name
- `COMMITBEE_OLLAMA_HOST` - Ollama server URL
- `COMMITBEE_API_KEY` - API key for cloud providers

## Supported Languages (tree-sitter)

Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, Ruby, C#

All 10 languages are individually feature-gated (`lang-rust`, `lang-typescript`, `lang-javascript`, `lang-python`, `lang-go`, `lang-java`, `lang-c`, `lang-cpp`, `lang-ruby`, `lang-csharp`) and enabled by default. Build with `--no-default-features --features lang-rust,lang-go` to include only specific languages.

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
    ├── git.rs           # GitService (gix + git CLI, concurrent content fetching)
    ├── analyzer.rs      # AnalyzerService (tree-sitter queries, parallel via rayon)
    ├── context.rs       # ContextBuilder (token budget)
    ├── history.rs       # HistoryService (commit style learning)
    ├── safety.rs        # Secret scanning (25 patterns), conflict detection
    ├── sanitizer.rs     # CommitSanitizer (JSON + plain text, BREAKING CHANGE footer)
    ├── splitter.rs      # CommitSplitter (multi-commit detection)
    ├── template.rs      # TemplateService (custom prompt templates)
    ├── progress.rs      # Progress indicators (indicatif spinners, TTY-aware)
    └── llm/
        ├── mod.rs       # LlmProvider trait + enum dispatch + shared SYSTEM_PROMPT
        ├── ollama.rs    # OllamaProvider (streaming NDJSON)
        ├── openai.rs    # OpenAiProvider (SSE streaming)
        └── anthropic.rs # AnthropicProvider (SSE streaming)
```

## References

- **PRD & Roadmap**: `PRD.md`
- **Conventional Commits spec anchoring**: `.claude/plans/PLAN_CONVENTIONAL_COMMITS_SPEC.md`
- **v0.3.0 enhancement plan**: `.claude/plans/PLAN_V030_ENHANCEMENTS.md`
- **Hunk-level splitting discussion**: [GitHub Discussion #2](https://github.com/Sephyi/commitbee/discussions/2)

## Project Skills

| Skill | Invocation | Purpose |
| --- | --- | --- |
| `ci-check` | `/ci-check [fast\|full\|test <name>]` | Run fmt + clippy + tests + audit |
| `reuse-annotate` | `/reuse-annotate <file>` | Add SPDX headers to new files |

## Project Agents

| Agent | File | Purpose |
| --- | --- | --- |
| `rust-security-reviewer` | `.claude/agents/rust-security-reviewer.md` | Read-only security audit (8-category) |
| `cargo-dep-auditor` | `.claude/agents/cargo-dep-auditor.md` | Check deps for outdated versions, yanked crates, advisories |
| `api-compat-reviewer` | `.claude/agents/api-compat-reviewer.md` | Check public API changes for breaking callers/impls |
| `llm-prompt-quality-reviewer` | `.claude/agents/llm-prompt-quality-reviewer.md` | Audit SYSTEM_PROMPT, schemas, CommitType sync, spec compliance |

## Project Hooks

| Hook | Trigger | Action |
| --- | --- | --- |
| `rust-fmt.sh` | PostToolUse Edit/Write | `rustfmt <file>` on `.rs` files |
| `block-generated-files.sh` | PreToolUse Edit/Write | Block manual edits to `Cargo.lock` |
| `superpowers-check.sh` | SessionStart | Warn if superpowers plugin missing |

## Development Notes

### Toolchain

- Rust edition 2024, MSRV 1.94
- License: PolyForm-Noncommercial-1.0.0 (REUSE compliant)
- Dev deps: `tempfile`, `assert_cmd`, `predicates`, `wiremock`, `insta`, `proptest`, `toml`

### REUSE / SPDX Headers

- All files use `reuse annotate` format: blank comment separator between SPDX lines
- `reuse lint` — verify compliance
- `reuse annotate --copyright "Sephyi <me@sephy.io>" --license PolyForm-Noncommercial-1.0.0 --year 2026 <file>` — add header
- REUSE.toml `[[annotations]]` — for files that can't have inline headers (Cargo.lock, tests/snapshots/**)

### Running Tests

```bash
cargo test                    # All tests (308 tests)
cargo test --test sanitizer   # CommitSanitizer tests
cargo test --test safety      # Safety module tests
cargo test --test context     # ContextBuilder tests
cargo test --test commit_type # CommitType tests
cargo test --test integration # LLM provider integration tests (wiremock)
cargo test --test languages  # Language-specific tree-sitter tests
cargo test --test history    # Commit history style learning tests
cargo test --test template   # Custom prompt template tests
cargo test -- --nocapture     # Show println output
```

**Important:** `cargo test sanitizer` matches test *names* across all binaries. Use `cargo test --test <name>` to select a specific integration test file.

### Test Conventions

- Async tests: `#[tokio::test]` (not `#[test]` with `.block_on()`)
- Snapshots: after changing output, run `cargo insta review` to accept/reject
- Snapshot env: `UPDATE_EXPECT=1 cargo test` for bulk snapshot update
- Wiremock: NDJSON streaming mocks use `respond_with(ResponseTemplate::new(200).set_body_raw(...))` with `\n`-delimited JSON
- Git fixtures: `tempfile::TempDir` + `git init` via `std::process::Command`, not real repos
- Proptest: `PROPTEST_CASES=1000` for thorough local runs before push

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

### Dependency Management

When adding or updating crates:
1. Verify latest stable version via `cargo search <crate> --limit 1` before adding to `Cargo.toml`
2. If a pre-release version is detected or would be added: **STOP and ask the user** — report the pre-release version found, the latest stable version (if any exists), and whether no stable release is available yet. Do not add a pre-release version without explicit user approval.
3. Prefer `x.y` (minor-compatible) over `=x.y.z` (exact pin) unless a bug requires it
4. Run `cargo audit` before and after adding new dependencies
5. Use `cargo-dep-auditor` agent for full pre-release dependency review

### LLM Provider Conventions

When adding or modifying LLM providers (`src/services/llm/`), every provider must:

1. **`new()` returns `Result<Self>`** — propagate HTTP client build errors, never `unwrap_or_default()`
2. **Import and check `MAX_RESPONSE_BYTES`** — cap `full_response.len()` inside the streaming loop to prevent unbounded memory growth
3. **Error body propagation** — use `unwrap_or_else(|e| format!("(failed to read body: {e})"))` on error response body reads, not `unwrap_or_default()`
4. **EOF buffer parsing** — after the byte stream ends, parse any remaining content in `line_buffer` (SSE streams may deliver the final frame without a trailing newline)
5. **Zero-allocation streaming** — parse from `&line_buffer[..newline_pos]` slices, then `drain(..=newline_pos)` instead of allocating new Strings per line
6. **Shared system prompt** — use `super::SYSTEM_PROMPT`, never duplicate prompt text
7. **CancellationToken** — check in `tokio::select!` loop alongside stream chunks

### Commit Type Conventions

Follow Conventional Commits strictly — the type must reflect what actually happened:

- **`fix`**: Corrects incorrect behavior (a bug existed, now it doesn't)
- **`feat`**: Adds a new capability or safeguard that didn't exist before (even defensive checks)
- **`refactor`**: Improves code without changing behavior (better error messages, code quality, documentation)
- **`perf`**: Measurable performance improvement

Common mistake: calling a new safeguard/check `fix` — if there was no bug, it's `feat`. Improving error message quality without changing control flow is `refactor`, not `fix`.

### Gotchas

- `gix` API: use `repo.workdir()` not `repo.work_dir()` (deprecated)
- `CommitType::parse()` not `from_str()` — avoids clippy `should_implement_trait` warning
- Enum variants used only via `CommitType::ALL` const need `#[allow(dead_code)]`
- Parallel subagents running `cargo fmt` may create unstaged changes — commit formatting separately
- Secret patterns: `sk-[a-zA-Z0-9]{48}` (legacy) and `sk-proj-[a-zA-Z0-9\-_]{40,}` (modern) — test data must match the exact format
- `tokio::process::Command` output needs explicit `std::process::Output` type annotation when using `.ok()?`
- Tree-sitter is CPU-bound/sync — pre-fetch file content into HashMaps async, then pass `&HashMap<PathBuf, String>` to `extract_symbols()` which uses rayon for parallel parsing
- `rayon::par_iter()` requires data to be `Sync`; `tree_sitter::Parser` is neither `Send` nor `Sync` — create a new `Parser` per file inside the rayon closure
- `#[cfg(feature = "secure-storage")]` gates both the error variant and CLI commands for keyring

### Known Issues

- **Non-atomic split commits**: The split flow uses `unstage_all → stage_files → commit` per group with no rollback. If an intermediate commit fails, earlier commits remain. Documented via TOCTOU comment in `app.rs`. Future improvement: index snapshot with full rollback (see [GitHub Discussion #2](https://github.com/Sephyi/commitbee/discussions/2)).
- **No streaming during split generation**: When commit splitting generates per-group messages, LLM output is not streamed to the terminal (tokens are consumed silently). Single-commit generation streams normally. Low priority — split generation is fast since each sub-prompt is smaller.
- **Thinking model output**: Models with thinking enabled prepend `<think>...</think>` blocks before their JSON response. The sanitizer strips both `<think>` and `<thought>` blocks (closed and unclosed) during parsing. The `think` config option (default: `false`) controls whether Ollama's thinking separation is used. The default model `qwen3.5:4b` does not use thinking mode and works well with the default `num_predict: 256`.
- **No think-then-compress**: Explicit `<thought>` prompting is not used — small models (<10B) exhaust their token budget on analysis instead of JSON output. The pre-computed EVIDENCE/CONSTRAINTS/SYMBOLS sections serve this role. Revisit for 70B+/cloud APIs.
- **Retry**: `validate_and_retry()` runs up to 3 attempts (`MAX_RETRIES: 3`), logging each violation individually before retry. Future: prioritized violation ordering, per-group retry for split commits.

### Documentation Sync

Keep README.md test count in sync (currently 308).
