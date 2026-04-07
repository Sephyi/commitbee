<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial
-->

# Changelog

All notable changes to CommitBee are documented here.

## `v0.6.0-dev` — Deep Understanding (release candidate)

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
