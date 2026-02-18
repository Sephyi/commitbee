// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub mod ollama;

use crate::config::{Config, Provider};
use crate::error::Result;

/// Enum dispatch for LLM providers — avoids async-trait / dyn overhead.
pub enum LlmBackend {
    Ollama(ollama::OllamaProvider),
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
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Ollama(p) => p.name(),
        }
    }

    /// Verify provider connectivity and model availability
    pub async fn verify(&self) -> Result<()> {
        match self {
            Self::Ollama(p) => p.verify_model().await,
        }
    }
}

pub fn create_provider(config: &Config) -> Result<LlmBackend> {
    match config.provider {
        Provider::Ollama => Ok(LlmBackend::Ollama(ollama::OllamaProvider::new(config))),
        Provider::OpenAI => {
            // TODO: Implement OpenAI provider
            Err(crate::error::Error::Provider {
                provider: "openai".into(),
                message: "OpenAI provider not yet implemented".into(),
            })
        }
        Provider::Anthropic => {
            // TODO: Implement Anthropic provider
            Err(crate::error::Error::Provider {
                provider: "anthropic".into(),
                message: "Anthropic provider not yet implemented".into(),
            })
        }
    }
}
