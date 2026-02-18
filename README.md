<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
-->

# 🐝 CommitBee

**The commit message generator that actually understands your code.**

CommitBee is a Rust-native CLI tool that uses **tree-sitter semantic analysis** and LLMs to generate high-quality [conventional commit](https://www.conventionalcommits.org/) messages. Unlike every other tool in this space, CommitBee doesn't just pipe raw `git diff` output to an LLM — it parses both the staged and HEAD versions of your files, maps diff hunks to symbol spans (functions, classes, methods), and provides structured semantic context. This produces fundamentally better commit messages, especially for complex multi-file changes.

> [!IMPORTANT]
> This project is in early development. If you're not planning to actively contribute toward the first stable release, I'd recommend waiting until a release is published before adopting it. The first release will signal that the project is ready for general use.

## ✨ What Makes CommitBee Different

| Feature                            | CommitBee | Others          |
| ---------------------------------- | --------- | --------------- |
| 🌳 Tree-sitter semantic analysis   | **Yes**   | No              |
| 🔒 Built-in secret scanning        | **Yes**   | Rarely          |
| 📊 Token budget management         | **Yes**   | No              |
| ⚡ Streaming LLM output            | **Yes**   | Rarely          |
| 🔍 Prompt debug mode               | **Yes**   | No              |
| 🏠 Local-first (Ollama default)    | **Yes**   | Cloud-first     |
| 🦀 Single static binary            | **Yes**   | Node.js/Python  |

Every competitor sends raw diffs to LLMs. CommitBee sends **semantic context** — which functions changed, what was added or removed, and why the change matters structurally.

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

- **Rust** 1.85+ (edition 2024)
- **Ollama** running locally (default provider) — [Install Ollama](https://ollama.ai)
- A model pulled in Ollama (recommended: `qwen3:4b`)

```bash
ollama pull qwen3:4b
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

## 🔧 Configuration

CommitBee stores configuration in a platform-specific directory. Create a config with:

```bash
commitbee init
```

### Example config

```toml
provider = "ollama"
model = "qwen3:4b"
ollama_host = "http://localhost:11434"
max_diff_lines = 500
max_file_lines = 100
max_context_chars = 24000

[format]
include_body = true
include_scope = true
lowercase_subject = true
```

### Environment variables

| Variable                 | Description              | Default                    |
| ------------------------ | ------------------------ | -------------------------- |
| `COMMITBEE_PROVIDER`     | LLM provider             | `ollama`                   |
| `COMMITBEE_MODEL`        | Model name               | `qwen3:4b`                 |
| `COMMITBEE_OLLAMA_HOST`  | Ollama server URL        | `http://localhost:11434`   |
| `COMMITBEE_API_KEY`      | API key (cloud providers)| —                          |

## 📖 Usage

```bash
commitbee [OPTIONS] [COMMAND]
```

### Options

| Flag              | Description                            |
| ----------------- | -------------------------------------- |
| `--dry-run`       | Print message only, don't commit       |
| `--yes`           | Auto-confirm and commit                |
| `-n, --generate`  | Generate N candidates (1-5, default 1) |
| `--verbose`       | Show symbol extraction details         |
| `--show-prompt`   | Debug: display the full LLM prompt     |

### Commands

| Command               | Description                            |
| --------------------- | -------------------------------------- |
| `init`                | Create a config file                   |
| `config`              | Show current configuration             |
| `doctor`              | Check configuration and connectivity   |
| `completions <shell>` | Generate shell completions             |
| `hook install`        | Install prepare-commit-msg hook        |
| `hook uninstall`      | Remove prepare-commit-msg hook         |
| `hook status`         | Check if hook is installed             |

## 🌳 How It Works

CommitBee's pipeline goes beyond simple diff forwarding:

```txt
┌─────────┐    ┌──────────┐    ┌────────────┐    ┌───────────┐    ┌─────────┐
│  Stage  │ →  │   Git    │ →  │ Tree-sitter│ →  │  Context  │ →  │   LLM   │
│ Changes │    │  Service │    │  Analyzer  │    │  Builder  │    │Provider │
└─────────┘    └──────────┘    └────────────┘    └───────────┘    └─────────┘
                    │                │                  │               │
               Staged diff      Symbol spans      Budget-aware     Commit message
               + file list      (functions,       prompt with      (conventional
                                classes, etc.)    semantic context    format)
```

1. **Git Service** — Discovers the repo, reads staged changes and diffs
2. **Tree-sitter Analyzer** — Parses both staged and HEAD file versions, maps diff hunks to symbol spans (functions, structs, methods)
3. **Context Builder** — Assembles a budget-aware prompt with file breakdown, semantic symbols, inferred commit type/scope, and truncated diff
4. **Safety Scanner** — Checks for secrets and merge conflicts before anything leaves your machine
5. **LLM Provider** — Streams the prompt to your chosen model and parses the response
6. **Commit Sanitizer** — Validates the output as proper conventional commit format (JSON or plain text)

### Supported languages

| Language     | Parser                   |
| ------------ | ------------------------ |
| Rust         | `tree-sitter-rust`       |
| TypeScript   | `tree-sitter-typescript` |
| JavaScript   | `tree-sitter-javascript` |
| Python       | `tree-sitter-python`     |
| Go           | `tree-sitter-go`         |

Files in unsupported languages are still included in the diff context — they just don't get semantic symbol extraction.

## 🔒 Security

CommitBee scans all content before it's sent to any LLM provider:

- 🔑 **API key detection** — AWS keys, OpenAI keys, generic secrets
- 🔐 **Private key detection** — PEM-encoded private keys
- 🔗 **Connection string detection** — Database URLs with credentials
- ⚠️ **Merge conflict detection** — Prevents committing unresolved conflicts

The default provider (Ollama) runs entirely on your machine. No data leaves your network unless you explicitly configure a cloud provider.

## 🏗️ Architecture

```bash
src/
├── main.rs              # Entry point
├── lib.rs               # Library exports
├── app.rs               # Application orchestrator
├── cli.rs               # CLI arguments (clap)
├── config.rs            # Configuration (figment layered)
├── error.rs             # Error types (thiserror + miette)
├── domain/
│   ├── change.rs        # FileChange, StagedChanges, ChangeStatus
│   ├── symbol.rs        # CodeSymbol, SymbolKind
│   ├── context.rs       # PromptContext (semantic prompt assembly)
│   └── commit.rs        # CommitType (single source of truth)
└── services/
    ├── git.rs           # GitService (gix + git CLI)
    ├── analyzer.rs      # AnalyzerService (tree-sitter)
    ├── context.rs       # ContextBuilder (token budget)
    ├── safety.rs        # Secret scanning, conflict detection
    ├── sanitizer.rs     # CommitSanitizer (JSON + plain text)
    └── llm/
        ├── mod.rs       # LlmProvider trait + enum dispatch
        ├── ollama.rs    # OllamaProvider (streaming NDJSON)
        ├── openai.rs    # OpenAiProvider (SSE streaming)
        └── anthropic.rs # AnthropicProvider (SSE streaming)
```

## 🧪 Testing

```bash
cargo test                    # All tests (101 tests)
cargo test --test sanitizer   # CommitSanitizer tests
cargo test --test safety      # Secret scanner tests
cargo test --test context     # ContextBuilder tests
cargo test --test commit_type # CommitType tests
cargo test --test integration # LLM provider integration tests
```

The test suite includes snapshot tests ([insta](https://insta.rs/)), property-based tests ([proptest](https://proptest-rs.github.io/proptest/)), never-panic guarantees for all user-facing parsers, and integration tests using [wiremock](https://docs.rs/wiremock) for LLM provider mocking.

## 🗺️ Roadmap

| Phase                       | Version    | Status           |
| --------------------------- | ---------- | ---------------- |
| 🔧 Stability & Correctness  | `v0.2.0`   | ✅ Complete       |
| ✨ Polish & Providers       | `v0.3.0`   | 🚧 In Progress   |
| 🚀 Differentiation          | `v0.4.0`   | 📋 Planned       |
| 👑 Market Leadership        | `v1.0+`    | 🔮 Future        |

### v0.3.0 highlights (in progress)

- **Cloud providers** — OpenAI-compatible and Anthropic streaming support
- **Git hook integration** — `commitbee hook install/uninstall/status`
- **Shell completions** — bash, zsh, fish, powershell via `clap_complete`
- **Rich error diagnostics** — `miette` for actionable error messages
- **Multiple message generation** — `--generate N` with interactive candidate selection
- **Hierarchical config** — `figment`-based layering (CLI > Env > File > Defaults)
- **Structured logging** — `tracing` with `COMMITBEE_LOG` env filter
- **Doctor command** — `commitbee doctor` for connectivity and config checks
- **Secure key storage** — OS keychain via `keyring` (optional feature)

See [`PRD.md`](PRD.md) for the full product requirements document.

## 🤝 Contributing

Contributions are welcome! By contributing, you agree to the [Contributor License Agreement](CLA.md) — you'll be asked to sign it when you open your first pull request.

The project uses:

- **Rust edition 2024** (MSRV 1.85)
- **Conventional commits** for all commit messages
- **REUSE/SPDX** for license compliance

```bash
# Development workflow
cargo fmt                     # Format code
cargo clippy -- -D warnings   # Lint (must pass clean)
cargo test                    # Run all tests

# Manual testing
git add some-file.rs
cargo run -- --dry-run        # Preview commit message
cargo run -- --show-prompt    # Debug the LLM prompt
```

## 💛 Sponsor

If you find CommitBee useful, consider [sponsoring my work](https://github.com/sponsors/Sephyi).

## 📄 License

This project is licensed under [PolyForm-Noncommercial-1.0.0](LICENSES/PolyForm-Noncommercial-1.0.0.txt).

REUSE compliant — every file carries SPDX headers.

Copyright 2026 [Sephyi](https://sephy.io)
