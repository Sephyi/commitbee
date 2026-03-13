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

pub struct OllamaProvider {
    client: Client,
    host: String,
    model: String,
    temperature: f32,
    num_predict: u32,
    think: bool,
}

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    system: String,
    stream: bool,
    think: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Deserialize)]
struct GenerateResponse {
    #[serde(default)]
    response: String,
    done: bool,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    name: String,
}

impl OllamaProvider {
    pub fn new(config: &Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| Error::Provider {
                provider: "ollama".into(),
                message: format!("failed to build HTTP client: {e}"),
            })?;

        Ok(Self {
            client,
            // Sanitize: remove trailing slashes to avoid //api/generate
            host: config.ollama_host.trim_end_matches('/').to_string(),
            model: config.model.clone(),
            temperature: config.temperature,
            num_predict: config.num_predict,
            think: config.think,
        })
    }

    /// Check Ollama connectivity and return available model names
    pub async fn health_check(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.host);

        let response = self.client.get(&url).send().await.map_err(|e| {
            if e.is_connect() {
                Error::OllamaNotRunning {
                    host: self.host.clone(),
                }
            } else {
                Error::Provider {
                    provider: "ollama".into(),
                    message: e.to_string(),
                }
            }
        })?;

        let tags: TagsResponse = response.json().await.map_err(|e| Error::Provider {
            provider: "ollama".into(),
            message: format!("failed to parse /api/tags response: {e}"),
        })?;

        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }

    /// Verify that the configured model is available
    pub async fn verify_model(&self) -> Result<()> {
        let available = self.health_check().await?;

        // Ollama model names may include `:latest` tag
        let model_matches = available.iter().any(|name| {
            name == &self.model
                || name == &format!("{}:latest", self.model)
                || name.strip_suffix(":latest") == Some(&self.model)
        });

        if !model_matches {
            return Err(Error::ModelNotFound {
                model: self.model.clone(),
                available,
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
        let url = format!("{}/api/generate", self.host);

        let response = self
            .client
            .post(&url)
            .json(&GenerateRequest {
                model: self.model.clone(),
                prompt: prompt.to_string(),
                system: SYSTEM_PROMPT.to_string(),
                stream: true,
                think: self.think,
                options: OllamaOptions {
                    temperature: self.temperature,
                    num_predict: self.num_predict,
                },
            })
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    Error::OllamaNotRunning {
                        host: self.host.clone(),
                    }
                } else if e.is_timeout() {
                    Error::Provider {
                        provider: "ollama".into(),
                        message: "request timed out".into(),
                    }
                } else {
                    Error::Provider {
                        provider: "ollama".into(),
                        message: e.to_string(),
                    }
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|e| format!("(failed to read body: {e})"));
            return Err(Error::Provider {
                provider: "ollama".into(),
                message: format!("HTTP {}: {}", status, body),
            });
        }

        let mut stream = response.bytes_stream();
        let mut full_response = String::new();

        // CRITICAL: Buffer for handling chunk boundaries
        // Chunks from bytes_stream() are NOT aligned to newlines!
        let mut line_buffer = String::new();
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    return Err(Error::Cancelled);
                }
                chunk = stream.next() => {
                    let Some(chunk) = chunk else {
                        break; // Stream ended
                    };

                    let chunk = chunk.map_err(|e| Error::Provider {
                        provider: "ollama".into(),
                        message: e.to_string(),
                    })?;

                    // Append chunk to buffer
                    line_buffer.push_str(&String::from_utf8_lossy(&chunk));

                    // Process complete lines (newline-delimited JSON)
                    while let Some(newline_pos) = line_buffer.find('\n') {
                        // Parse from slice to avoid allocating a String per line
                        let result = {
                            let line = &line_buffer[..newline_pos];
                            if line.is_empty() {
                                None
                            } else {
                                serde_json::from_str::<GenerateResponse>(line).ok()
                            }
                        };
                        // Shift buffer in-place (no allocation)
                        line_buffer.drain(..=newline_pos);

                        if let Some(resp) = result {
                            let _ = token_tx.send(resp.response.clone()).await;
                            full_response.push_str(&resp.response);

                            if resp.done {
                                return Ok(full_response.trim().to_string());
                            }
                        }
                    }
                }
            }
        }

        // Handle any remaining content in buffer
        if !line_buffer.is_empty()
            && let Ok(resp) = serde_json::from_str::<GenerateResponse>(&line_buffer)
        {
            full_response.push_str(&resp.response);
        }

        Ok(full_response.trim().to_string())
    }

    pub fn name(&self) -> &str {
        "ollama"
    }
}
