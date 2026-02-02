use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::LlmProvider;
use crate::config::Config;
use crate::error::{Error, Result};

pub struct OllamaProvider {
    client: Client,
    host: String,
    model: String,
}

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
    done: bool,
}

impl OllamaProvider {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            // Sanitize: remove trailing slashes to avoid //api/generate
            host: config.ollama_host.trim_end_matches('/').to_string(),
            model: config.model.clone(),
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn generate(
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
                stream: true,
            })
            .send()
            .await
            .map_err(|e| Error::Provider {
                provider: "ollama".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
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
                        let line = line_buffer[..newline_pos].to_string();
                        line_buffer = line_buffer[newline_pos + 1..].to_string();

                        if line.is_empty() {
                            continue;
                        }

                        if let Ok(resp) = serde_json::from_str::<GenerateResponse>(&line) {
                            // Send token for streaming display
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
        if !line_buffer.is_empty() {
            if let Ok(resp) = serde_json::from_str::<GenerateResponse>(&line_buffer) {
                full_response.push_str(&resp.response);
            }
        }

        Ok(full_response.trim().to_string())
    }

    fn name(&self) -> &str {
        "ollama"
    }
}
