<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial
-->

# Changelog

All notable changes to CommitBee are documented here.

## `v0.7.0-dev` — Unreleased

### CLI / UX

- **Machine-readable output mode (`--porcelain`)** — Print only the sanitized commit message to stdout for piping into other tools (scripts, editors, git hooks, higher-level automation). All spinners, live-streamed LLM JSON, info/warning lines, tracing output, and ANSI styling are silenced; errors still flow to stderr with a non-zero exit for reliable script detection. Implies `--dry-run` and `--no-split`; rejected at argument-parse time when combined with `--yes`, `--clipboard`, `--show-prompt`, `--verbose`, `-n/--generate`, or any subcommand. Closes #6.
- **Documented `--allow-secrets` risk in help text** — The flag's clap help now leads with `DANGER:` and explains that detection is *not* bypassed; the flag only enables an interactive confirmation, and non-interactive modes (`--yes`, `--porcelain`, piped stdin) still fail closed. Audit F-019.

### Security & Safety

- **Non-blocking `--allow-secrets` under `--yes` / `--porcelain`** — The interactive "Send diff to LLM anyway?" confirmation now skips to the non-interactive "fail closed" branch when either `--yes` or `--porcelain` is set, preventing pipelines from silently hanging on piped stdin when secrets are detected.
- **Loopback-only `ollama_host`** — Config validation now rejects any `ollama_host` whose URL host is not loopback (`127.0.0.0/8` IPv4 range, `::1` IPv6, or the literal string `localhost`). Rejects any path/query/fragment as well, since the Ollama provider concatenates the host with `/api/...` and a non-empty path would silently forward unintended segments. Implements PRD SR-001. Audit F-030.
- **Binary crate enforces `#![forbid(unsafe_code)]`** — `src/main.rs` now imports `App`, `Cli`, and `error` from the library crate (`commitbee::*`) instead of re-declaring the module tree, so the safety attribute applies to everything the binary compiles. Audit F-028.
- **Doctor no longer echoes any portion of the API key** — `commitbee doctor` previously printed `(key '****1234')` on a successful provider verify. Even partial disclosure correlates keys across logs/screenshots; the suffix is now removed entirely. Audit F-029 follow-up.
- **`reqwest` pinned to `rustls`** — `default-features = false` plus the `rustls` feature gives an explicit TLS pin instead of relying on whatever default-tls reqwest 0.13 happens to ship. Audit F-005.

### Reliability & Observability

- **Cloud provider verification in `doctor`** — `commitbee doctor` now calls `provider.verify().await` for OpenAI/Anthropic providers, not just an `is_some()` API-key check. Audit F-029.
- **Async hook and clipboard paths** — `hook_dir`/`hook_path`/`hook_install`/`hook_uninstall`/`hook_status`/`handle_hook` are now `async fn` and use `tokio::process::Command` for `git rev-parse`. `copy_to_clipboard` runs `arboard` inside `tokio::task::spawn_blocking` (the library is sync). Audit F-002.
- **Hermetic `git` calls in history tests** — `tests/history.rs` builds tempdir-backed git repos with `GIT_CONFIG_NOSYSTEM=1`, an empty `GIT_CONFIG_GLOBAL` file (works on Windows), `HOME` redirected to the tempdir, and pre-supplied author/committer identity, so tests pass under hosts that have GPG signing or a missing `user.email`. Audit F-032.
- **Tracing spans on pipeline functions** — `App::generate_commit`, `AnalyzerService::extract_symbols`, and `ContextBuilder::build` are now `#[tracing::instrument(skip_all, fields(...))]` with scalar metadata only (no diff text, no `SecretString`). Audit F-013.
- **Logged JoinSet panics in `git show` fetch** — `GitService::fetch_file_contents` now matches on `Err(JoinError)` and emits `tracing::warn!` instead of silently dropping the result. Spawned tasks carry a `tracing::warn_span!("git_show", path = …)` so the log line preserves the path that the spawn closure consumed. Audit F-008.
- **Log Ctrl+C handler install failures** — The spawned signal-handler task no longer discards registration errors via `.ok()`; on `Err` it logs via `tracing::warn!` and still calls `cancel.cancel()` so any CancellationToken-aware task receives a shutdown signal. Audit F-025.
- **Rounded percentage conversion in history prompt** — `to_prompt_section()` now rounds the type-distribution percentages (e.g., 2/3 → 67%) instead of truncating via `as u32`. Audit F-027.

### Performance

- **Cached built-in secret regex set** — `BUILTIN_PATTERNS: LazyLock<Vec<SecretPattern>>` compiles all built-in regexes exactly once per process. `build_patterns()` clones from the cached slice instead of recompiling on every call. Disabled-pattern lookup uses a `HashSet<String>` rather than `Vec::contains`. Audit F-012.
- **`RegexSet` first-pass for secret scanning** — `PatternSet` wraps the built-in patterns with a derived `regex::RegexSet`. Each scanned line goes through one combined NFA traversal; the per-pattern `Regex::is_match` is consulted only on a hit. `BUILTIN_PATTERN_SET: LazyLock<PatternSet>` caches the set. Audit F-022.
- **Streamed whitespace-stripped comparison** — `ContextBuilder::classify_span_change`, `classify_span_change_rich`, and `AstDiffer::bodies_semantically_equal` no longer allocate two `String`s to drop whitespace; they now stream filtered `char` iterators through `Iterator::eq()`. Audit F-021.
- **Bounded concurrent `git show` spawning** — `GitService::fetch_file_contents` now acquires a `tokio::sync::Semaphore` permit before each `git show` task. The bound scales as `cores * 2`, clamped to `2..=32` so single-core hosts no longer fork the previous hard floor of 16 processes. Audit F-020.

### Code Quality

- **`replace byte indexing with `strip_prefix`** — `&line[1..]` byte slicing in `src/services/{context,splitter}.rs` is replaced with `line.strip_prefix('+')` / `strip_prefix('-')`, removing the multi-byte-UTF-8 panic surface. Audit F-011.
- **Hunk-header detection no longer false-skips real content** — Diff scanners that previously dropped any line starting with `+++` / `---` (which incidentally also skipped legitimate added/removed content lines whose body began with `++` or `--`) now track `in_hunk` after `@@` and treat synthetic test diffs (no `@@`) as fully in-hunk. Affects `splitter::diff_fingerprint`, `context::detect_import_changes`, `context::detect_intents`, `context::detect_metadata_breaking`, `context::detect_bug_evidence`, the cross-file call detector, and the dependency version detector. Audit F-011 follow-up.
- **`#[forbid(unsafe_code)]` on the binary crate** — see Security above. Audit F-028.
- **`SAFETY:` comments on cast truncations** — Every `#[allow(clippy::cast_possible_truncation)]` site in `analyzer.rs` and `differ.rs` now carries an inline `// SAFETY:` comment justifying why the truncation cannot lose information at runtime. Audit F-015.

### CI

- **macOS in the matrix** — `clippy` and `test` jobs now fan out to `[ubuntu-24.04, macos-14]` (pinned to a specific macOS image to avoid silent moves when GitHub advances `macos-latest`). Audit F-016 / F-016 follow-up.
- **Dedicated MSRV (1.94) job** — A separate workflow job runs the floor toolchain explicitly with `dtolnay/rust-toolchain@master` so `cargo check --all-features --all-targets` truly exercises MSRV. Audit F-007.
- **`cargo-deny` license / advisory / source enforcement** — A new `deny` job runs `EmbarkStudios/cargo-deny-action@v2 check --all-features` after installing the project's pinned Rust toolchain. `deny.toml` defines an allow-list of permissive licenses, an exception for the dual-licensed root crate (`AGPL-3.0-only OR LicenseRef-Commercial`), bans on duplicate-major-version pulls and unknown registries/git sources, and a v2 `[advisories]` section. Implements PRD SR-005. Audit F-017.
- **`clippy.toml` disallowed-methods / disallowed-macros** — Project-wide clippy config bans `std::process::Command::new` (sync, blocks the runtime) and `std::dbg` (leftover scaffolding). Test files that need a sync `Command` for tempdir-backed fixtures opt out via narrow `#[allow]` annotations on the specific helper. Audit F-018.

### Testing

- **`make_symbol` helper parametrised by line range** — `tests/helpers.rs` now exposes `make_symbol(name, kind, file, is_public, is_added)` (defaults `line: 1, end_line: 10`) and a `make_symbol_at(...)` variant for tests that need to pin a specific line range to a hand-crafted diff hunk. Test files dropped their per-file copies. Audit D-040.
- **`ChangeStatus::Deleted` and `ChangeStatus::Renamed` covered in fixtures** — New tests in `tests/{splitter,context}.rs` exercise both variants through the splitter, the file-breakdown formatter, and the rename-marker render path. Includes `make_renamed_file_with_diff` for fixtures that need an explicit diff body. Audit D-037.
- **Coverage for the `None` branch of `classify_span_change`** — Three new tests in `tests/context.rs` directly assert the `Option::None` short-circuit (outside-hunk span, empty diff, inverted span overlapping the hunk). Audit D-039.
- **Multi-language fuzz target for signature extraction** — New `fuzz_signature_multilang` cargo-fuzz target dispatches input via `data[0] % 10` to all 10 supported tree-sitter grammars (Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, Ruby, C#) through `commitbee::extract_*_signature` wrappers. Uses `String::from_utf8_lossy` to keep coverage on arbitrary byte input. Audit D-047.
- **437 tests** total (down from 442 nominal at v0.6.0 due to test consolidation during the audit merge — net new tests added by audit batch are partially offset by deduplicated coverage).

### Internal

- **Removed unsafe `std::env::set_var("NO_COLOR")`** — The previous SAFETY justification did not hold under `#[tokio::main(rt-multi-thread)]` (worker threads are already spawned by the time `main`'s body executes). Color suppression is now handled entirely through `console::set_colors_enabled(false)`, porcelain-aware `miette` `GraphicalTheme::unicode_nocolor()`, and the tracing subscriber's `.with_ansi(false)`.
- **Porcelain-aware `miette` diagnostic rendering** — `miette::set_hook` now installs after `Cli::parse()` and receives `terminal_links(!porcelain)` + `GraphicalTheme::unicode_nocolor()` under porcelain mode, so errors on stderr stay free of OSC8 hyperlinks and ANSI escapes regardless of external `NO_COLOR` state.

## `v0.6.0` — Semantic Intelligence

### UI/UX

- **Interactive message refinement** — Added a "Refine" option to the candidate selection and confirmation menu. Users can provide feedback to the LLM (e.g., "more detail about the API change") to regenerate the message with natural language guidance.
- **Native clipboard support** — Replaced external commands (`pbcopy`, `xclip`) with the `arboard` crate for a native, cross-platform clipboard implementation.
- **Interactive commit editing** — Added an "Edit" option to the candidate selection and confirmation menu. Users can now refine the generated message using their system editor before committing.

### Semantic Analysis

- **Full AST diffs for structs and enums** — `AstDiffer` now supports structured diffing for structs, enums, classes, interfaces, and traits. Detects added/removed fields, changed variants, and type modifications.
- **Parent scope extraction** — Methods inside `impl`, `class`, or `trait` blocks now show their parent in the prompt: `CommitValidator > pub fn validate(...)`. Walks the AST tree through intermediate nodes (declaration_list, class_body). Verified across 7 languages (Rust, Python, TypeScript, Java, Go, Ruby, C#).
- **Import change detection** — New `IMPORTS CHANGED:` prompt section shows added/removed import statements. Supports Rust `use`, JS/TS `import`, Python `from`/`import`, Node `require()`, and C/C++ `#include`. Capped at 10 entries.
- **Doc-vs-code distinction** — `SpanChangeKind` enum classifies modified symbols as WhitespaceOnly, DocsOnly, Mixed, or Semantic. Doc-only changes suggest `docs` type. Modified symbols show `[docs only]` or `[docs + code]` suffix in the prompt.
- **Test file correlation** — New `RELATED FILES:` prompt section shows when source files and their matching test files are both staged. Stem-based matching, capped at 5 entries.
- **Structural AST diffs** — `AstDiffer` compares old and new tree-sitter nodes for modified symbols, producing structured `SymbolDiff` descriptions (parameter added, return type changed, visibility changed, async toggled, body modified). Shown as `STRUCTURED CHANGES:` section in the prompt.
- **Whitespace-aware body comparison** — Body diff uses character-stream stripping so reformatting doesn't produce false `BodyModified` results.
- **Structured changes in prompt** — New `STRUCTURED CHANGES:` section in the LLM prompt shows concise one-line descriptions of what changed per symbol (e.g., `CommitValidator::validate(): +param strict: bool, return bool → Result<()>, body modified`). Omitted when no structural diffs exist.
- **Semantic markers** — `AstDiffer` now detects `unsafe` added/removed, `#[derive()]` changes, decorator additions/removals, export changes, mutability changes, and generic constraint changes. Shown as `+unsafe`, `+derive(Debug, Clone)`, etc. in the STRUCTURED CHANGES section.

### Type Inference

- **Test-to-code ratio** — When >80% of additions are in test files, suggests `test` type even with source files present. Uses cross-multiplication to avoid integer truncation.

### Change Intent Detection

- **Diff-based intent patterns** — Scans added lines for error handling (`Result`, `?`, `Err()`), test additions (`#[test]`, `assert!`), logging (`tracing::`, `debug!`), and dependency updates. Shown as `INTENT:` section in the prompt with confidence scores.
- **Conservative type refinement** — High-confidence performance optimization patterns can override the base type to `perf`.

### Security

- **Accurate secret scan line numbers** — The secret scanner now parses `@@` hunk headers to report accurate source line numbers for potential secrets, instead of absolute diff line numbers.
- **API key validation ordering** — `set-key`, `get-key`, `init`, `config`, `completions`, and `hook` commands no longer require an API key to be present. CLI `--provider` flag now applies before keyring lookup.
- **Platform-native keyring backends** — keyring v3 now uses macOS Keychain (`apple-native`), Windows Credential Manager (`windows-native`), and Linux Secret Service (`linux-native`) instead of a mock file-based backend.
- **SecretString for API keys** — API keys stored as `secrecy::SecretString` in Config and provider structs. Memory zeroed on drop, never exposed except at HTTP header insertion.
- **Overflow checks in release builds** — `overflow-checks = true` added to release profile for ANSSI-FR compliance.

### Performance

- **Optimized symbol dependency merging** — Improved `CommitSplitter` performance for large commits by pre-indexing symbols and optimizing diff scanning.

### Prompt Quality

- **Token budget rebalance** — Symbol budget reduced from 30% to 20% when structural diffs are available, freeing space for the raw diff. SYSTEM_PROMPT updated to guide the LLM to prefer STRUCTURED CHANGES for signature details.
- **Unsafe constraint rule** — When `unsafe` is added to a function, a CONSTRAINTS rule instructs the LLM to mention safety justification in the commit body.

### Testing

- **442 tests** total (up from 367 at v0.5.0).

## `v0.5.0` — Beyond the Diff

### Semantic Analysis

- **Full signature extraction** — The LLM sees `pub fn connect(host: &str, timeout: Duration) -> Result<Connection>`, not just "Function connect." Two-strategy body detection: `child_by_field_name("body")` primary, `BODY_NODE_KINDS` fallback. Works across all 10 languages.
- **Signature diffs for modified symbols** — When a function signature changes, the prompt shows `[~] old_sig → new_sig`.
- **Cross-file connection detection** — Detects when a changed file calls a symbol defined in another changed file. Shown as `CONNECTIONS: validator calls parse() — both changed`.
- **Semantic change classification** — Modified symbols classified as whitespace-only or semantic via character-stream comparison. Formatting-only changes auto-detected as `style`.
- **Dual old/new line tracking** — Correctly handles symbols shifting positions between HEAD and staged.
- **Token budget rebalance** — Symbol section gets 30% of budget (up from 20%) when signatures present.

### Security

- **Block project config URL overrides** — `.commitbee.toml` can no longer redirect `openai_base_url`, `anthropic_base_url`, or `ollama_host` to prevent SSRF/exfiltration of API keys and staged code.
- **Cap streaming line_buffer** — All 3 LLM providers cap `line_buffer` at 1 MB to prevent unbounded memory growth from malicious servers.
- **Strip URLs from error messages** — `reqwest::Error` display uses `without_url()` to prevent leaking configured base URLs.
- **Broadened OpenAI secret pattern** — Detects `sk-proj-` and `sk-svcacct-` prefixed keys alongside legacy `sk-` format.
- **Replaced Box::leak with Cow** — Custom secret pattern names use `Cow<'static, str>` instead of leaked heap allocations.

### Prompt Quality

- **Fixed breaking change subject budget** — Subject character budget now accounts for `!` suffix, preventing guaranteed validator rejection on breaking changes.
- **Omit empty EVIDENCE section** — Saves ~200 chars when all flags are at default (most changes).
- **Symbol marker legend** — SYSTEM_PROMPT now explains `[+] added, [-] removed, [~] modified`.
- **Removed duplicate JSON schema** — System prompt no longer includes a competing schema template.
- **Replaced emoji with text** — `⚠` replaced with `WARNING:` for better small-model tokenization.
- **Enhanced Python queries** — Tree-sitter now captures decorated functions and classes.

### Testing & Evaluation

- **Evaluation harness** — 36 fixtures covering all 11 commit types, AST features, and edge cases. Per-type accuracy reporting with `EvalSummary`.
- **15+ new unit tests** — Coverage for `detect_primary_change`, `detect_metadata_breaking`, `detect_bug_evidence` (all 7 patterns), Deleted/Renamed status, signature edge cases, connection content assertions.
- **5 fuzz targets** — `fuzz_sanitizer`, `fuzz_safety`, `fuzz_diff_parser`, `fuzz_signature`, `fuzz_classify_span`.
- **367 tests** total (up from 308 at v0.4.0).

### API

- **Demoted internal types** — `SymbolChangeType`, `GitService`, `Progress` changed from `pub` to `pub(crate)`.
- **Added `#[non_exhaustive]`** to `SymbolChangeType` for future-safe extension.

## `v0.4.0` — See Everything

- **10-language tree-sitter support** — Added Java, C, C++, Ruby, and C# to the existing Rust, TypeScript, JavaScript, Python, and Go. All languages are individually feature-gated and enabled by default. Disable any with `--no-default-features` + selective `--features lang-rust,lang-go,...`.
- **Custom prompt templates** — User-defined templates with `{{diff}}`, `{{symbols}}`, `{{files}}`, `{{type}}`, `{{scope}}` variables via `template_path` config.
- **Multi-language commit messages** — Generate messages in any language with `--locale` flag or `locale` config (e.g., `--locale de` for German).
- **Commit history style learning** — Learns from recent commit history to match your project's style (`learn_from_history`, `history_sample_size` config).
- **Rename detection** — Detects file renames with similarity percentage via `git diff --find-renames`, displayed as `old → new (N% similar)` in prompts and split suggestions. Configurable threshold (default 70%, set to 0 to disable).
- **Expanded secret scanning** — 24 built-in patterns across 13 categories (cloud providers, AI/ML, source control, communication, payment, database, cryptographic, generic). Pluggable engine: add custom regex patterns or disable built-ins by name via config.
- **Progress indicators** — Contextual `indicatif` spinners during pipeline phases (analyzing, scanning, generating). Auto-suppressed in non-TTY environments (git hooks, pipes).
- **Evaluation harness** — `cargo test --features eval` for structured LLM output quality benchmarking.
- **Fuzz testing** — `cargo-fuzz` targets for sanitizer and diff parser robustness.
- **Exclude files** — `--exclude <GLOB>` flag (repeatable) and `exclude_patterns` config option. Glob patterns filter files from analysis (e.g., `*.lock`, `**/*.generated.*`, `vendor/**`). CLI patterns additive with config.
- **Copy to clipboard** — `--clipboard` flag copies the generated message to the system clipboard and prints to stdout, skipping commit confirmation.

## `v0.3.1` — Trust, but Verify

- **Multi-pass corrective retry** — Validator checks LLM output against 7 rules and retries up to 3 times with targeted correction instructions
- **Subject length enforcement** — Rejects subjects exceeding 72-char first line with a clear error instead of silent truncation
- **Stronger prompt budget** — Character limit embedded directly in JSON template, "HARD LIMIT" phrasing for better small-model compliance
- **Default model: `qwen3.5:4b`** — Smaller (3.4GB), no thinking overhead, clean JSON output out of the box
- **Configurable thinking mode** — `think` config option for Ollama models that support reasoning separation

## `v0.3.0` — Read Between the Lines

- **Diff-shape fingerprinting + Jaccard clustering** — Splitter groups files by change shape and content vocabulary, not just directory
- **Evidence-based type inference** — Constraint rules from code analysis drive commit type selection (bug evidence → fix, mechanical → style, dependency-only → chore)
- **Robust LLM output parsing** — Sanitizer handles `<think>`/`<thought>` blocks, conversational preambles, noisy JSON extraction
- **Metadata-aware breaking change detection** — Detects MSRV bumps, engines.node, requires-python changes
- **Symbol tri-state tracking** — Added/removed/modified-signature differentiation in tree-sitter analysis
- **Primary change detection** — Identifies the single most significant change for subject anchoring
- **Post-generation validation** — Subject specificity validator ensures concrete entity naming
- **NUL-delimited git parsing** — Safe handling of paths with special characters
- **Parallel tree-sitter parsing** — rayon for CPU-bound parsing, tokio JoinSet for concurrent git fetching
- **Anti-hallucination prompt engineering** — EVIDENCE/CONSTRAINTS sections, negative examples, anti-copy rules

## `v0.2.0` — Commit, Don't Think

- **Cloud providers** — OpenAI-compatible and Anthropic streaming support
- **Commit splitting** — Automatic detection and splitting of multi-concern staged changes
- **Git hook integration** — `commitbee hook install/uninstall/status`
- **Shell completions** — bash, zsh, fish, powershell via `clap_complete`
- **Rich error diagnostics** — `miette` for actionable error messages
- **Multiple message generation** — `--generate N` with interactive candidate selection
- **Hierarchical config** — `figment`-based layering (CLI > Env > File > Defaults)
- **Structured logging** — `tracing` with `COMMITBEE_LOG` env filter
- **Doctor command** — `commitbee doctor` for connectivity and config checks
- **Secure key storage** — OS keychain via `keyring` (optional feature)
- **Body line wrapping** — Commit body text wrapped at 72 characters
