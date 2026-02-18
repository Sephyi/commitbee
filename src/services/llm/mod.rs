// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
