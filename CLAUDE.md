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
- **Streaming**: Line-buffered JSON parsing with CancellationToken

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
    ├── git.rs           # GitService (gix + git CLI, concurrent content fetching)
    ├── analyzer.rs      # AnalyzerService (tree-sitter, parallel via rayon)
    ├── context.rs       # ContextBuilder (token budget)
    ├── safety.rs        # Secret scanning, conflict detection
    ├── sanitizer.rs     # CommitSanitizer (JSON + plain text, BREAKING CHANGE footer)
    ├── splitter.rs      # CommitSplitter (multi-commit detection)
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
- **Implementation plan (v1, outdated)**: `.claude/plans/PLAN_COMMITBEE_V1.md` — superseded by PRD v2.1
- **Skills ecosystem design**: `.claude/plans/2026-02-22-skills-ecosystem-design.md`

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
cargo test                    # All tests (182 tests)
cargo test --test sanitizer   # CommitSanitizer tests
cargo test --test safety      # Safety module tests
cargo test --test context     # ContextBuilder tests
cargo test --test commit_type # CommitType tests
cargo test --test integration # LLM provider integration tests (wiremock)
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

### Gotchas

- `gix` API: use `repo.workdir()` not `repo.work_dir()` (deprecated)
- `CommitType::parse()` not `from_str()` — avoids clippy `should_implement_trait` warning
- Enum variants used only via `CommitType::ALL` const need `#[allow(dead_code)]`
- Parallel subagents running `cargo fmt` may create unstaged changes — commit formatting separately
- Secret pattern `sk-[a-zA-Z0-9]{48}` requires exactly 48 chars after `sk-` in test data
- `tokio::process::Command` output needs explicit `std::process::Output` type annotation when using `.ok()?`
- Tree-sitter is CPU-bound/sync — pre-fetch file content into HashMaps async, then pass `&HashMap<PathBuf, String>` to `extract_symbols()` which uses rayon for parallel parsing
- `rayon::par_iter()` requires data to be `Sync`; `tree_sitter::Parser` is neither `Send` nor `Sync` — create a new `Parser` per file inside the rayon closure
- `#[cfg(feature = "secure-storage")]` gates both the error variant and CLI commands for keyring

### Known Issues

- **No streaming during split generation**: When commit splitting generates per-group messages, LLM output is not streamed to the terminal (tokens are consumed silently). Single-commit generation streams normally. Low priority — split generation is fast since each sub-prompt is smaller.
- **Thinking model output**: Models with thinking enabled prepend `<think>...</think>` blocks before their JSON response. The sanitizer strips both `<think>` and `<thought>` blocks (closed and unclosed) during parsing. The `think` config option (default: `false`) controls whether Ollama's thinking separation is used. The default model `qwen3.5:4b` does not use thinking mode and works well with the default `num_predict: 256`.
- **Think-then-Compress prompting**: Evaluated and removed in v0.3.0. Adding `<thought>` instructions to prompts caused small models (<10B) to spend their token budget on analysis text instead of JSON output. The pre-computed EVIDENCE/CONSTRAINTS/SYMBOLS sections already do the "thinking" for the model. **Future consideration**: revisit for larger models (70B+, cloud APIs) where chain-of-thought genuinely improves output quality — would require bumping `num_predict` to 512+ and careful prompt engineering to keep thinking concise.
- **Retry improvement plan**: Current retry is single-pass (one correction attempt via `validate_and_retry()`). **Future improvement**: configurable `max_retries` (default 3), prioritized violation ordering — fix critical errors first (e.g., `breaking_change` detection, invalid type), then structural issues, then length shortening last. Per-group retry for split commits. Would require a new config field and loop in `validate_and_retry()`.

### Post-Implementation Documentation TODOs

- **README.md Running Tests**: Kept in sync with test count updates (currently 182).
