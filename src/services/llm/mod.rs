// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

// SYNC: commit type list must match CommitType::ALL in src/domain/commit.rs
pub(crate) const SYSTEM_PROMPT: &str = r#"You generate Conventional Commit messages from git diffs.

Use exactly one type:
feat, fix, refactor, chore, docs, test, style, perf, build, ci, revert

Only set "breaking_change" if existing users or dependents must change their code, config,
or scripts to keep working — e.g., a public function/type removed or renamed, a required
parameter added, a config key renamed. New optional additions, bug fixes, and internal
refactors are NOT breaking. Default to null.

Rules:
- Subject: imperative, specific, lowercase start, no trailing period, max 72 chars total first line.
- Body: 1-3 sentences about WHY for non-trivial changes, else null.
- Do not list files changed.

Output ONLY valid JSON (nullable fields use null, not the string "null"):
{"type":"<type>","scope":null,"subject":"<subject>","body":null,"breaking_change":null}
For scope, body, and breaking_change: replace null with a quoted string when applicable.
"#;

pub mod anthropic;
pub mod ollama;
pub mod openai;

use crate::config::{Config, Provider};
use crate::error::Result;

/// Enum dispatch for LLM providers — avoids async-trait / dyn overhead.
pub enum LlmBackend {
    Ollama(ollama::OllamaProvider),
    OpenAi(openai::OpenAiProvider),
    Anthropic(anthropic::AnthropicProvider),
}

impl LlmBackend {
    /// Generate with streaming tokens and cancellation support
    pub async fn generate(
        &self,
        prompt: &str,
        token_tx: mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<String> {
        match self {
            Self::Ollama(p) => p.generate(prompt, token_tx, cancel).await,
            Self::OpenAi(p) => p.generate(prompt, token_tx, cancel).await,
            Self::Anthropic(p) => p.generate(prompt, token_tx, cancel).await,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Ollama(p) => p.name(),
            Self::OpenAi(p) => p.name(),
            Self::Anthropic(p) => p.name(),
        }
    }

    /// Verify provider connectivity and model availability
    pub async fn verify(&self) -> Result<()> {
        match self {
            Self::Ollama(p) => p.verify_model().await,
            Self::OpenAi(p) => p.verify_connection().await,
            Self::Anthropic(p) => p.verify_connection().await,
        }
    }
}

pub fn create_provider(config: &Config) -> Result<LlmBackend> {
    match config.provider {
        Provider::Ollama => Ok(LlmBackend::Ollama(ollama::OllamaProvider::new(config))),
        Provider::OpenAI => Ok(LlmBackend::OpenAi(openai::OpenAiProvider::new(config))),
        Provider::Anthropic => Ok(LlmBackend::Anthropic(anthropic::AnthropicProvider::new(
            config,
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::SYSTEM_PROMPT;
    use crate::domain::CommitType;

    #[test]
    fn system_prompt_type_list_matches_commit_type_all() {
        let types_line = SYSTEM_PROMPT
            .lines()
            .find(|line| line.contains("feat, fix, refactor"))
            .expect("SYSTEM_PROMPT must contain the commit type list line");

        let found: Vec<&str> = types_line
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        assert_eq!(
            found,
            CommitType::ALL,
            "SYSTEM_PROMPT type list must match CommitType::ALL exactly (order matters)"
        );
    }
}
