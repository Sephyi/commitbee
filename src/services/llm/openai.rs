// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::error::{Error, Result};

use super::SYSTEM_PROMPT;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAiProvider {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

impl OpenAiProvider {
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: config
                .openai_base_url
                .clone()
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            model: config.model.clone(),
            api_key: config.api_key.clone().unwrap_or_default(),
            temperature: config.temperature,
            max_tokens: config.num_predict,
        }
    }

    pub async fn verify_connection(&self) -> Result<()> {
        let url = format!("{}/models", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| Error::Provider {
                provider: "openai".into(),
                message: e.to_string(),
            })?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Provider {
                provider: "openai".into(),
                message: "invalid API key".into(),
            });
        }

        Ok(())
    }

    pub async fn generate(
        &self,
        prompt: &str,
        token_tx: mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&ChatRequest {
                model: self.model.clone(),
                messages: vec![
                    Message {
                        role: "system".into(),
                        content: SYSTEM_PROMPT.into(),
                    },
                    Message {
                        role: "user".into(),
                        content: prompt.to_string(),
                    },
                ],
                temperature: self.temperature,
                max_tokens: self.max_tokens,
                stream: true,
            })
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    Error::Provider {
                        provider: "openai".into(),
                        message: "request timed out".into(),
                    }
                } else {
                    Error::Provider {
                        provider: "openai".into(),
                        message: e.to_string(),
                    }
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Provider {
                provider: "openai".into(),
                message: format!("HTTP {status}: {body}"),
            });
        }

        let mut stream = response.bytes_stream();
        let mut full_response = String::new();
        let mut line_buffer = String::new();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    return Err(Error::Cancelled);
                }
                chunk = stream.next() => {
                    let Some(chunk) = chunk else { break };

                    let chunk = chunk.map_err(|e| Error::Provider {
                        provider: "openai".into(),
                        message: e.to_string(),
                    })?;

                    line_buffer.push_str(&String::from_utf8_lossy(&chunk));

                    while let Some(newline_pos) = line_buffer.find('\n') {
                        let line = line_buffer[..newline_pos].to_string();
                        line_buffer = line_buffer[newline_pos + 1..].to_string();

                        let line = line.trim();
                        if line.is_empty() || line == "data: [DONE]" {
                            continue;
                        }

                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };

                        if let Ok(chunk) = serde_json::from_str::<ChatChunk>(data) {
                            for choice in &chunk.choices {
                                if let Some(ref content) = choice.delta.content {
                                    let _ = token_tx.send(content.clone()).await;
                                    full_response.push_str(content);
                                }
                                if choice.finish_reason.is_some() {
                                    return Ok(full_response.trim().to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(full_response.trim().to_string())
    }

    pub fn name(&self) -> &str {
        "openai"
    }
}
