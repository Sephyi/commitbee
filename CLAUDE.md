<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial
-->

# CommitBee

AI-powered commit message generator using tree-sitter semantic analysis and local LLMs.

## Quick Start

```bash
cargo build --release
./target/release/commitbee
```

## Architecture

**Pipeline:** git diff → tree-sitter parse → symbol extraction + structural diffing → context building (budget, evidence, connections, imports, intents) → LLM prompt → sanitize → validate+retry → commit

- **Hybrid Git**: gix for repo discovery, git CLI for diffs (documented choice)
- **Tree-sitter**: Full file parsing with hunk mapping (not just +/- lines)
- **Parallelism**: rayon for CPU-bound tree-sitter parsing, tokio JoinSet for concurrent git content fetching
- **LLM**: Ollama primary (qwen3.5:4b), OpenAI/Anthropic secondary
- **Streaming**: Line-buffered JSON parsing with CancellationToken, 1 MB response cap (`MAX_RESPONSE_BYTES`)

## Key Design Decisions

1. **Full file parsing** - Parse staged/HEAD blobs, map diff hunks to symbol spans
2. **Token budget** - 24K char limit (~6K tokens), prioritizes diff over symbols
3. **TTY detection** - Safe for git hooks (graceful non-interactive fallback)
4. **Commit sanitizer** - Validates LLM output (JSON + plain text), emits `BREAKING CHANGE:` footer regardless of `include_body`
5. **Structured JSON output** - Prompt requests JSON for reliable parsing; schema includes `breaking_change: Option<String>` field
6. **System prompt** - Single `SYSTEM_PROMPT` in `llm/mod.rs`, shared by all providers. Type list synced with `CommitType::ALL`, 72-char subject limit.
7. **Simplified user prompt** - Concise format optimized for <4B parameter models
8. **Commit splitting** - Detects multi-concern changes, suggests splitting into separate commits
9. **Body line wrapping** - Sanitizer wraps body text at 72 characters
10. **Signature extraction** - Two-strategy: `child_by_field_name("body")` primary, `BODY_NODE_KINDS` fallback, first-line final fallback. 200-char cap with `floor_char_boundary`. No `.scm` query changes needed.
11. **Semantic change classification** - Modified symbols classified via character-stream comparison (not bag-of-lines). `build()` restructured: classify → infer_commit_type → format.
12. **Cross-file connections** - `detect_connections` scans added diff lines for `sym_name(` patterns. Min 4-char name filter, capped at 5, sort+dedup.
13. **Parent scope extraction** - `extract_parent_scope` walks up AST through intermediate nodes (declaration_list, class_body) to find impl/class/trait. 7 languages.
14. **Structural AST diffs** - `AstDiffer` compares old/new tree-sitter nodes for modified symbols. Returns owned `SymbolDiff` (no Node lifetime leaks). Runs inside `extract_for_file()` while both Trees alive.
15. **Change intent detection** - `detect_intents` scans diff lines for error handling, test, logging, dependency patterns. Threshold >2 matches. Conservative type refinement (only overrides for `perf`).
16. **Doc-vs-code classification** - `SpanChangeKind` enum (WhitespaceOnly, DocsOnly, Mixed, Semantic). Doc-only symbols suggest `docs` type. `is_doc_comment()` uses line-prefix heuristic.
17. **Adaptive token budget** - Symbol budget 20% with structural diffs, 30% with signatures only, 20% base.

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

## References

- **PRD & Roadmap**: `PRD.md`
- **Implementation plans**: `.claude/plans/` (gitignored, local only)
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
- License: AGPL-3.0-only OR LicenseRef-Commercial (dual-license, REUSE compliant)
- Dev deps: `tempfile`, `assert_cmd`, `predicates`, `wiremock`, `insta`, `proptest`, `toml`

### REUSE / SPDX Headers

- All files use `reuse annotate` format: blank comment separator between SPDX lines
- `reuse lint` — verify compliance
- `reuse annotate --copyright "Sephyi <me@sephy.io>" --license "AGPL-3.0-only OR LicenseRef-Commercial" --year 2026 <file>` — add header
- REUSE.toml `[[annotations]]` — for files that can't have inline headers (Cargo.lock, tests/snapshots/**)

### Running Tests

```bash
cargo test                    # All tests (424 tests)
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

# Test commit message generation with debug logging (shows validation retries)
COMMITBEE_LOG=debug ./target/release/commitbee --dry-run
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
8. **SecretString for API keys** — store as `secrecy::SecretString`, expose only via `.expose_secret()` at HTTP header insertion. Never log, Debug, or Display the raw key.

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
- Subagents dispatched without Bash permission can't commit — commit in the main session after verifying their changes
- Parallel subagents touching the same file will conflict — only parallelize when files don't overlap
- `SymbolKey` uses `(kind, name, file)` — do NOT add `line` (lines shift between HEAD/staged, breaks modified-symbol matching)
- `classify_span_change` uses new-file line range — old-file lines may differ when code shifts; known limitation (deferred #9)
- `extract_symbols()` returns `(Vec<CodeSymbol>, Vec<SymbolDiff>)` — all callers must destructure or use `.0`
- `ChangeDetail` has 25 variants (15 structural + 10 semantic markers) — keep `format_short()` in sync when adding new ones
- `infer_commit_type` takes `all_modified_docs_only: bool` parameter — must be computed in `build()` before calling

### Known Issues

- **Non-atomic split commits**: The split flow uses `unstage_all → stage_files → commit` per group with no rollback. If an intermediate commit fails, earlier commits remain. Documented via TOCTOU comment in `app.rs`. Future improvement: index snapshot with full rollback (see [GitHub Discussion #2](https://github.com/Sephyi/commitbee/discussions/2)).
- **No streaming during split generation**: When commit splitting generates per-group messages, LLM output is not streamed to the terminal (tokens are consumed silently). Single-commit generation streams normally. Low priority — split generation is fast since each sub-prompt is smaller.
- **Thinking model output**: Models with thinking enabled prepend `<think>...</think>` blocks before their JSON response. The sanitizer strips both `<think>` and `<thought>` blocks (closed and unclosed) during parsing. The `think` config option (default: `false`) controls whether Ollama's thinking separation is used. The default model `qwen3.5:4b` does not use thinking mode and works well with the default `num_predict: 256`.
- **No think-then-compress**: Explicit `<thought>` prompting is not used — small models (<10B) exhaust their token budget on analysis instead of JSON output. The pre-computed EVIDENCE/CONSTRAINTS/SYMBOLS sections serve this role. Revisit for 70B+/cloud APIs.
- **Retry**: `validate_and_retry()` runs up to 3 attempts (`MAX_RETRIES: 3`), logging each violation individually before retry. Future: prioritized violation ordering, per-group retry for split commits.

### Commit Generation Test Results

Real-world test results are tracked in auto-memory at `test-results.md`. After every manual test of commit message generation (`commitbee --dry-run`), record:

- The staged changes (files, type of change)
- Expected vs actual commit type
- Subject and body quality assessment
- Prompt observations (signatures, connections, evidence flags)
- Any issues (retry warnings, display bugs, misclassifications)

Compare new tests against previous results to detect regressions or improvements. The goal is generating fantastic commit messages with small local LLMs (qwen3.5:4b).

### Deferred Issues

A tracked list of review findings, design decisions, and improvement ideas that were identified but deferred lives in auto-memory at `deferred-issues.md`. Rules:

- **Check the list** when starting work on a related area, before releases, and at PRD updates
- **Add new items** when deferring anything from a review, plan, or implementation — every deferred item must be recorded with source, context, and "when to address" criteria
- **Never silently defer** — when deferring issues, explicitly tell the user what is being deferred, why, and when it should be revisited. Present deferred items as decisions that need user acknowledgment, not as internal bookkeeping
- **Close items** by updating status to `Done` with date when addressed

### Documentation Sync

Test counts and version references must stay in sync across multiple files. After adding/removing tests or bumping version:

- **README.md** — test count in features list + testing section + changelog current version
- **DOCS.md** — test count in description + testing section
- **PRD.md** — test count in §2.3 (feature status table), §8 (testing header), §11 (roadmap table), §12 (success metrics). Also: PRD version header, changelog entry, and compatibility policy table on version bumps.
- **CHANGELOG.md** — test count in current version's testing section
- **CLAUDE.md** — test count in Running Tests section

Use `cargo test --all-targets 2>&1 | grep "^test result" | awk '{sum += $4} END {print sum}'` to get the actual count. Don't guess from memory — counts drift easily.
