<!--
SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>

SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial
-->

# Security Policy

## Reporting a Vulnerability

**Do not open a public issue.**

**Preferred:** Use GitHub's private vulnerability reporting — click
**"Report a vulnerability"** on the
[Security tab](../../security/advisories/new) of this repository. This
creates a private advisory draft with a CVE workflow.

**Alternative:** Email [me@sephy.io](mailto:me@sephy.io) with details.

Include as much detail as possible:

- Description of the vulnerability
- Steps to reproduce
- Affected component (LLM providers, secret scanning, git operations, config)
- Potential impact

You will receive an acknowledgment within 7 days. Fixes for confirmed
vulnerabilities will be released as patch versions with a security advisory.

## Scope

Security issues in the following areas are in scope:

- **LLM streaming** — buffer exhaustion, response size limits, malformed server responses
- **Secret scanning** — pattern bypass, false negatives on known key formats
- **Config security** — project-level config overrides that could redirect API traffic or exfiltrate data
- **Git operations** — command injection via file paths, argument injection
- **Error messages** — credential or URL leakage in error output
- **Prompt injection** — crafted diff content that manipulates LLM behavior to produce harmful output
- **Dependency vulnerabilities** — reqwest, tokio, tree-sitter, serde

## Out of Scope

- LLM output quality (wrong commit type, generic subjects) — use the issue tracker
- Feature requests — use the issue tracker
- Vulnerabilities in Ollama, OpenAI, or Anthropic services themselves
