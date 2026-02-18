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

const BASE_URL: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    client: Client,
    model: String,
    api_key: String,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    system: String,
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
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<ContentDelta>,
}

#[derive(Deserialize)]
struct ContentDelta {
    text: Option<String>,
}

const SYSTEM_PROMPT: &str = r#"You are a commit message generator. Analyze git diffs and output JSON commit messages.

RULES:
1. Read the diff carefully - describe the ACTUAL changes you see
2. The subject must be SPECIFIC - mention what was added/changed/fixed
3. Output ONLY valid JSON
4. Start subject with lowercase verb: add, fix, update, remove, refactor

BAD: "describe what changed" or "update code"
GOOD: "add rate limiting to api endpoints" or "fix null check in user service""#;

impl AnthropicProvider {
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();

        Self {
            client,
            model: config.model.clone(),
            api_key: config.api_key.clone().unwrap_or_default(),
            temperature: config.temperature,
            max_tokens: config.num_predict,
        }
    }

    pub async fn verify_connection(&self) -> Result<()> {
        // Anthropic doesn't have a lightweight endpoint for verification,
        // so we just validate that the key looks plausible
        if self.api_key.is_empty() {
            return Err(Error::Provider {
                provider: "anthropic".into(),
                message: "API key not configured".into(),
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
        let url = format!("{BASE_URL}/messages");

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&MessagesRequest {
                model: self.model.clone(),
                system: SYSTEM_PROMPT.into(),
                messages: vec![Message {
                    role: "user".into(),
                    content: prompt.to_string(),
                }],
                temperature: self.temperature,
                max_tokens: self.max_tokens,
                stream: true,
            })
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    Error::Provider {
                        provider: "anthropic".into(),
                        message: "request timed out".into(),
                    }
                } else {
                    Error::Provider {
                        provider: "anthropic".into(),
                        message: e.to_string(),
                    }
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Provider {
                provider: "anthropic".into(),
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
                        provider: "anthropic".into(),
                        message: e.to_string(),
                    })?;

                    line_buffer.push_str(&String::from_utf8_lossy(&chunk));

                    while let Some(newline_pos) = line_buffer.find('\n') {
                        let line = line_buffer[..newline_pos].to_string();
                        line_buffer = line_buffer[newline_pos + 1..].to_string();

                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }

                        // SSE format: "event: <type>" followed by "data: <json>"
                        if line.starts_with("event:") {
                            continue;
                        }

                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };

                        if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                            match event.event_type.as_str() {
                                "content_block_delta" => {
                                    if let Some(delta) = &event.delta {
                                        if let Some(text) = &delta.text {
                                            let _ = token_tx.send(text.clone()).await;
                                            full_response.push_str(text);
                                        }
                                    }
                                }
                                "message_stop" => {
                                    return Ok(full_response.trim().to_string());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(full_response.trim().to_string())
    }

    pub fn name(&self) -> &str {
        "anthropic"
    }
}
