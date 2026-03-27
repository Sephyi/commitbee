<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial
-->

# 🐝 CommitBee &emsp; [![Build Status]][ci] [![MSRV]][rust-1.94] [![License]][license-file] [![Total Downloads]][crates-io]

[Build Status]: https://github.com/sephyi/commitbee/actions/workflows/ci.yml/badge.svg?branch=main
[ci]: https://github.com/sephyi/commitbee/actions/workflows/ci.yml
[MSRV]: https://img.shields.io/badge/MSRV-1.94-orange.svg
[rust-1.94]: https://blog.rust-lang.org/2026/03/05/Rust-1.94.0
[License]: https://img.shields.io/badge/license-AGPL--3.0-blue.svg
[license-file]: LICENSE
[Total Downloads]: https://img.shields.io/crates/d/commitbee?style=social&logo=iCloud&logoColor=black
[crates-io]: https://crates.io/crates/commitbee

**The commit message generator that actually understands your code.**

Most tools in this space pipe raw `git diff` to an LLM and hope for the best. CommitBee parses your code with [tree-sitter](https://tree-sitter.github.io/tree-sitter/), maps diff hunks to symbol spans, and gives the LLM structured semantic context — producing fundamentally better commit messages, especially for complex multi-file changes.

## ✨ What Sets CommitBee Apart

### 🌳 It reads your code, not just your diffs

CommitBee uses tree-sitter to parse both the staged and HEAD versions of every changed file — in parallel across CPU cores. It extracts 10 symbol types (functions, methods, structs, enums, traits, impls, classes, interfaces, constants, type aliases) with **full signatures** — the LLM sees `pub fn connect(host: &str, timeout: Duration) -> Result<Connection>`, not just "Function connect." Methods include their parent context (`impl Server > connect`), so the LLM knows *where* a symbol lives, not just its name. Modified symbols show old → new signature diffs, and **structural AST diffs** break down exactly what changed per symbol — parameters added, return type altered, visibility widened, or body-only edits. Cross-file relationships are detected automatically: if `validator.rs` calls `parse()` and both changed, the prompt says so. When source and test files are both staged, the prompt links them as related files. Symbols are tracked in three states: **added**, **removed**, and **modified-signature**.

Supported languages: **Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, Ruby, C#** — all enabled by default, individually toggleable via Cargo feature flags. Files in other languages still get full diff context — just without symbol extraction.

### 🧠 It reasons about what changed

Before the LLM generates anything, CommitBee computes deterministic evidence from your code and encodes it as hard constraints in the prompt:

- **Bug-fix evidence** in the diff → `fix`. No bug evidence → the LLM can't call it a `fix`.
- **Formatting-only changes** (whitespace, import reordering) → `style`. Detected both heuristically and via per-symbol whitespace classification.
- **Doc-vs-code distinction** — changes that only touch doc comments are classified separately from code changes, preventing `refactor` when `docs` is correct.
- **Import change detection** — added/removed imports are surfaced explicitly so the LLM can distinguish dependency wiring from logic changes.
- **Test-to-code ratio** — when >80% of additions are test code, the type is inferred as `test`.
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
- **🔒 Secret scanning** — 24 built-in patterns across 13 categories (cloud keys, AI/ML tokens, payment, database, crypto). Add custom patterns or disable built-ins via config.
- **⚡ Streaming** — Real-time token display from all 3 providers (Ollama, OpenAI, Anthropic) with Ctrl+C cancellation.
- **📊 Token budget** — Smart truncation that prioritizes the most important files within ~6K tokens. Automatically rebalances when structural diffs are available.
- **🎯 Multi-candidate** — Generate up to 5 messages and pick the best one interactively.
- **🪝 Git hooks** — `prepare-commit-msg` hook with TTY detection for safe non-interactive fallback.
- **🔍 Prompt debug** — `--show-prompt` shows exactly what the LLM sees. Full transparency.
- **🩺 Doctor** — `commitbee doctor` checks config, connectivity, and model availability.
- **🐚 Shell completions** — bash, zsh, fish, powershell via `commitbee completions`.
- **⚙️ 5-level config** — Defaults → project `.commitbee.toml` → user config → env vars → CLI flags.
- **🦀 Single binary** — ~18K lines of Rust. Compiles to one static binary with LTO. No runtime dependencies.
- **🧪 424 tests** — Unit, snapshot, property (proptest for never-panic guarantees), and integration (wiremock).

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
| `--clipboard` | Copy message to clipboard instead of committing |
| `--exclude <GLOB>` | Exclude files matching glob pattern (repeatable) |
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

CommitBee scans all content before it's sent to any LLM provider with **24 built-in patterns** across 13 categories:

- ☁️ **Cloud providers** — AWS access/secret keys, GCP service accounts & API keys, Azure storage keys
- 🤖 **AI/ML** — OpenAI, Anthropic, HuggingFace tokens
- 🔧 **Source control** — GitHub (PAT, fine-grained, OAuth), GitLab tokens
- 💬 **Communication** — Slack tokens & webhooks, Discord webhooks
- 💳 **Payment & SaaS** — Stripe, Twilio, SendGrid, Mailgun keys
- 🗄️ **Database** — MongoDB, PostgreSQL, MySQL, Redis, AMQP connection strings
- 🔐 **Cryptographic** — PEM private keys, JWT tokens
- 🔑 **Generic** — API key assignments, quoted/unquoted secrets
- ⚠️ **Merge conflict detection** — Prevents committing unresolved conflicts

Add custom patterns or disable built-ins in your config:

```toml
custom_secret_patterns = ["CUSTOM_KEY_[a-zA-Z0-9]{32}"]
disabled_secret_patterns = ["Generic Secret (unquoted)"]
```

The default provider (Ollama) runs entirely on your machine. No data leaves your network unless you explicitly configure a cloud provider.

## 🧪 Testing

```bash
cargo test   # 424 tests — unit, snapshot (insta), property (proptest), integration (wiremock)
```

See [Testing Strategy](DOCS.md#testing-strategy) for the full breakdown.

## 🗺️ Changelog

See [`CHANGELOG.md`](CHANGELOG.md) for the full version history.

**Current:** `v0.6.0-rc.1` *Deep Understanding* — Parent scope extraction, structural AST diffs, import change detection, doc-vs-code distinction, test file correlation, test-to-code ratio inference, adaptive token budgeting, semantic markers (unsafe, derive, decorator, export detection in AST diffs), and change intent detection (error handling, test, logging patterns with confidence scoring).

## 🤝 Contributing

Contributions are welcome! By contributing, you agree to the [Contributor License Agreement](CLA.md) — you'll be asked to sign it when you open your first pull request.

## 💛 Sponsor

If you find CommitBee useful, consider [**sponsoring my work**](https://github.com/sponsors/Sephyi) — it helps keep the project going.

## 📄 License

This project is dual-licensed under [AGPL-3.0-only](LICENSES/AGPL-3.0-only.txt) and a commercial license. See [LICENSE](LICENSE) for details.

REUSE compliant — every file carries SPDX headers.

Copyright 2026 [Sephyi](https://sephy.io)
