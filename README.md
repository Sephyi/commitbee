<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
-->

# 🐝 CommitBee &emsp; [![Build Status]][ci] [![MSRV]][rust-1.94] [![License]][license-file] [![REUSE Status]][reuse-info] ![Total Downloads]

[Build Status]: https://github.com/sephyi/commitbee/actions/workflows/ci.yml/badge.svg?branch=main
[ci]: https://github.com/sephyi/commitbee/actions/workflows/ci.yml
[MSRV]: https://img.shields.io/badge/MSRV-1.94-orange.svg
[rust-1.94]: https://blog.rust-lang.org/2025/06/26/Rust-1.94.0.html
[License]: https://img.shields.io/badge/license-PolyForm--Noncommercial-blue.svg
[license-file]: LICENSES/PolyForm-Noncommercial-1.0.0.txt
[REUSE Status]: https://api.reuse.software/badge/github.com/sephyi/commitbee
[reuse-info]: https://api.reuse.software/info/github.com/sephyi/commitbee
[Total Downloads]: https://img.shields.io/crates/d/commitbee?style=social&logo=iCloud&logoColor=black

**The commit message generator that actually understands your code.**

Most tools in this space pipe raw `git diff` to an LLM and hope for the best. CommitBee parses your code with [tree-sitter](https://tree-sitter.github.io/tree-sitter/), maps diff hunks to symbol spans, and gives the LLM structured semantic context — producing fundamentally better commit messages, especially for complex multi-file changes.

## ✨ What Sets CommitBee Apart

### 🌳 It reads your code, not just your diffs

CommitBee uses tree-sitter to parse both the staged and HEAD versions of every changed file — in parallel across CPU cores. It extracts 10 symbol types (functions, methods, structs, enums, traits, impls, classes, interfaces, constants, type aliases) and maps diff hunks to their spans. The LLM doesn't see "lines 42-58 changed" — it sees "the `validate()` function in `sanitizer.rs` was modified, and a new `retry()` method was added." Symbols are tracked in three states: **added**, **removed**, and **modified-signature**.

Supported languages: **Rust, TypeScript, JavaScript, Python, Go**. Files in other languages still get full diff context — just without symbol extraction.

### 🧠 It reasons about what changed

Before the LLM generates anything, CommitBee computes deterministic evidence from your code and encodes it as hard constraints in the prompt:

- **Bug-fix evidence** in the diff → `fix`. No bug evidence → the LLM can't call it a `fix`.
- **Formatting-only changes** (whitespace, import reordering) → `style`. Not `feat`, not `fix`.
- **Dependency-only changes** → `chore`. Always.
- **Public API removed** → breaking change flagged automatically.
- **MSRV bumps, `engines.node`, `requires-python` changes** → metadata-aware breaking detection.

Commit types are driven by code analysis, not LLM guesswork. The prompt includes computed EVIDENCE flags, CONSTRAINTS the model must follow, the primary change for subject anchoring, a character budget for the subject line, and anti-hallucination rules with negative examples.

### ✅ It validates and corrects its own output

Every generated message passes through a 7-rule validation pipeline:

1. Fix requires evidence — no bug comments, no `fix` type
2. Breaking change detection — removed public APIs must be flagged
3. Anti-hallucination — breaking change text can't copy internal field names
4. Mechanical changes must use `style`
5. Dependency-only changes must use `chore`
6. Subject specificity — rejects generic messages like "update code" or "improve things"
7. Subject length — enforces the 72-character first line limit

If any rule fails, CommitBee appends targeted correction instructions and re-prompts the LLM — up to 3 attempts, re-validating after each. The final output goes through a sanitizer that strips thinking blocks, extracts JSON from code fences, removes conversational preambles, and wraps the body at 72 characters. You get a clean, spec-compliant conventional commit or a clear error — never a silently mangled message.

### 🔀 It splits multi-concern commits

When your staged changes mix independent work (a bugfix in one module + a refactor in another), CommitBee detects it and offers to split them into separate, well-typed commits. The splitter uses diff-shape fingerprinting combined with Jaccard similarity on content vocabulary — files are grouped by the actual shape and language of their changes, not just by directory. Symbol dependency merging keeps related files together even when their diff shapes differ: if `foo()` is removed from one file and added in another, they stay in the same commit.

```txt
⚡ Commit split suggested — 2 logical change groups detected:

  Group 1: feat(llm)  [2 files]
    [M] src/services/llm/anthropic.rs (+20 -5)
    [M] src/services/llm/openai.rs (+8 -3)

  Group 2: fix(sanitizer)  [1 file]
    [M] src/services/sanitizer.rs (+3 -1)

? Split into separate commits? (Y/n)
```

### The pipeline

```txt
┌─────────┐    ┌──────────┐    ┌────────────┐    ┌──────────┐    ┌───────────┐    ┌─────────┐
│  Stage  │ →  │   Git    │ →  │ Tree-sitter│ →  │  Split   │ →  │  Context  │ →  │   LLM   │
│ Changes │    │  Service │    │  Analyzer  │    │ Detector │    │  Builder  │    │Provider │
└─────────┘    └──────────┘    └────────────┘    └──────────┘    └───────────┘    └─────────┘
                    │                │                 │                │               │
               Staged diff      Symbol spans     Group files      Budget-aware     Commit message
               + file list      (functions,      by module,       prompt with      (conventional
                                classes, etc.)   suggest split    semantic context    format)
```

### And there's more

- **🏠 Local-first** — Ollama by default. Your code never leaves your machine. No API keys needed.
- **🔒 Secret scanning** — Catches API keys, private keys, and connection strings before anything reaches the LLM.
- **⚡ Streaming** — Real-time token display from all 3 providers (Ollama, OpenAI, Anthropic) with Ctrl+C cancellation.
- **📊 Token budget** — Smart truncation that prioritizes the most important files within ~6K tokens.
- **🎯 Multi-candidate** — Generate up to 5 messages and pick the best one interactively.
- **🪝 Git hooks** — `prepare-commit-msg` hook with TTY detection for safe non-interactive fallback.
- **🔍 Prompt debug** — `--show-prompt` shows exactly what the LLM sees. Full transparency.
- **🩺 Doctor** — `commitbee doctor` checks config, connectivity, and model availability.
- **🐚 Shell completions** — bash, zsh, fish, powershell via `commitbee completions`.
- **⚙️ 5-level config** — Defaults → project `.commitbee.toml` → user config → env vars → CLI flags.
- **🦀 Single binary** — ~18K lines of Rust. Compiles to one static binary with LTO. No runtime dependencies.
- **🧪 182 tests** — Unit, snapshot, property (proptest for never-panic guarantees), and integration (wiremock).

## 📦 Installation

### From source

```bash
cargo install commitbee
```

### Build from repository

```bash
git clone https://github.com/sephyi/commitbee.git
cd commitbee
cargo build --release
```

The binary will be at `./target/release/commitbee`.

### Requirements

- **Rust** 1.94+ (edition 2024)
- **Ollama** running locally (default provider) — [Install Ollama](https://ollama.ai)
- A model pulled in Ollama (recommended: `qwen3.5:4b`)

```bash
ollama pull qwen3.5:4b
```

## 🚀 Quick Start

```bash
# Stage your changes
git add src/feature.rs

# Generate and commit interactively
commitbee

# Preview without committing
commitbee --dry-run

# Auto-confirm and commit
commitbee --yes

# See what the LLM sees
commitbee --show-prompt
```

That's it. CommitBee works with zero configuration if Ollama is running locally.

> If CommitBee saves you time, consider [**sponsoring the project**](https://github.com/sponsors/Sephyi) 💛

## 📖 Documentation

- **[Full Guide](DOCS.md)** — configuration, providers, splitting, validation, troubleshooting
- **[PRD & Roadmap](PRD.md)** — product requirements and future plans

## 🔧 Configuration

Run `commitbee init` to create a config file. Works out of the box with zero config if Ollama is running locally.

See [Configuration](DOCS.md#-configuration) for the full config reference, environment variables, and layering priority.

## 💻 Usage

```bash
commitbee [OPTIONS] [COMMAND]
```

### Options

| Flag | Description |
| --- | --- |
| `--dry-run` | Print message only, don't commit |
| `--yes` | Auto-confirm and commit |
| `-n, --generate` | Generate N candidates (1-5, default 1) |
| `--no-split` | Disable commit split suggestions |
| `--no-scope` | Disable scope in commit messages |
| `--allow-secrets` | Allow committing with detected secrets |
| `--verbose` | Show symbol extraction details |
| `--show-prompt` | Debug: display the full LLM prompt |

### Commands

| Command | Description |
| --- | --- |
| `init` | Create a config file |
| `config` | Show current configuration |
| `doctor` | Check configuration and connectivity |
| `completions <shell>` | Generate shell completions |
| `hook install` | Install prepare-commit-msg hook |
| `hook uninstall` | Remove prepare-commit-msg hook |
| `hook status` | Check if hook is installed |

## 🔒 Security

CommitBee scans all content before it's sent to any LLM provider:

- 🔑 **API key detection** — AWS keys, OpenAI keys, generic secrets
- 🔐 **Private key detection** — PEM-encoded private keys
- 🔗 **Connection string detection** — Database URLs with credentials
- ⚠️ **Merge conflict detection** — Prevents committing unresolved conflicts

The default provider (Ollama) runs entirely on your machine. No data leaves your network unless you explicitly configure a cloud provider.

## 🧪 Testing

```bash
cargo test   # 182 tests — unit, snapshot (insta), property (proptest), integration (wiremock)
```

See [Testing Strategy](DOCS.md#testing-strategy) for the full breakdown.

## 🗺️ Changelog

### 🔬 `v0.3.1` — Trust, but Verify (current)

- **Multi-pass corrective retry** — Validator checks LLM output against 7 rules and retries up to 3 times with targeted correction instructions
- **Subject length enforcement** — Rejects subjects exceeding 72-char first line with a clear error instead of silent truncation
- **Stronger prompt budget** — Character limit embedded directly in JSON template, "HARD LIMIT" phrasing for better small-model compliance
- **Default model: `qwen3.5:4b`** — Smaller (3.4GB), no thinking overhead, clean JSON output out of the box
- **Configurable thinking mode** — `think` config option for Ollama models that support reasoning separation

### 🚀 `v0.3.0` — Read Between the Lines

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

### ✨ `v0.2.0` — Commit, Don't Think

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

See [`PRD.md`](PRD.md) for the full product requirements document.

## 🤝 Contributing

Contributions are welcome! By contributing, you agree to the [Contributor License Agreement](CLA.md) — you'll be asked to sign it when you open your first pull request.

## 💛 Sponsor

If you find CommitBee useful, consider [**sponsoring my work**](https://github.com/sponsors/Sephyi) — it helps keep the project going.

## 📄 License

This project is licensed under [PolyForm-Noncommercial-1.0.0](LICENSES/PolyForm-Noncommercial-1.0.0.txt).

REUSE compliant — every file carries SPDX headers.

Copyright 2026 [Sephyi](https://sephy.io)
