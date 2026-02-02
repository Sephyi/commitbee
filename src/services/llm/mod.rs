// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
// SPDX-License-Identifier: GPL-3.0-only

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub mod ollama;

use crate::config::{Config, Provider};
use crate::error::Result;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate with streaming tokens and cancellation support
    async fn generate(
        &self,
        prompt: &str,
        token_tx: mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<String>;

    fn name(&self) -> &str;
}

pub fn create_provider(config: &Config) -> Result<Box<dyn LlmProvider>> {
    match config.provider {
        Provider::Ollama => Ok(Box::new(ollama::OllamaProvider::new(config))),
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
