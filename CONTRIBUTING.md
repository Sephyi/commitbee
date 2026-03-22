<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial
-->

# Contributing to CommitBee

Thank you for your interest in contributing! This guide covers the process
for submitting changes.

## Contributor License Agreement

All contributors must sign the [Contributor License Agreement](CLA.md)
before their pull request can be merged.

**How it works:**

1. Open a pull request.
2. The CLA bot will comment asking you to sign.
3. Reply with the signature phrase indicated by the bot.
4. The bot records your signature — you only need to sign once.

## Development Setup

```bash
# Clone and build
git clone https://github.com/sephyi/commitbee.git
cd commitbee
cargo build

# Run tests
cargo test --all-targets

# Run with eval harness
cargo test --all-targets --features eval

# Lint
cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

**Requirements:** Rust 1.94+ (edition 2024), Ollama for manual testing.

## Before Submitting

1. Run the full CI gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets`
2. Add tests for new functionality
3. Follow existing code patterns and conventions (see [CLAUDE.md](CLAUDE.md) for details)
4. Keep commits focused — one logical change per commit
5. Use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages

## Code Style

- `cargo fmt` for formatting (enforced by CI)
- `cargo clippy -- -D warnings` for linting (zero warnings required)
- See the [TypeScript style guide](https://ts.dev/style) principles adapted for Rust in CLAUDE.md

## REUSE Compliance

All files must have SPDX headers. Use:

```bash
reuse annotate --copyright "Sephyi <me@sephy.io>" --license "AGPL-3.0-only OR LicenseRef-Commercial" --year 2026 <file>
```

## Reporting Bugs

Use the [bug report template](../../issues/new?template=bug_report.yml).
Include `commitbee --version`, `commitbee doctor` output, and
`commitbee --dry-run --show-prompt` output when relevant.

## Requesting Features

Use the [feature request template](../../issues/new?template=feature_request.yml).

## Security Issues

See [SECURITY.md](SECURITY.md) — do not open public issues for vulnerabilities.
