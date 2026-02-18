// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! Integration tests for LLM providers and sanitizer pipeline.
//!
//! Uses `wiremock` to mock HTTP endpoints so no real LLM servers are needed.

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use commitbee::config::{CommitFormat, Config, Provider};
use commitbee::error::Error;
use commitbee::services::llm::anthropic::AnthropicProvider;
use commitbee::services::llm::ollama::OllamaProvider;
use commitbee::services::llm::openai::OpenAiProvider;
use commitbee::services::sanitizer::CommitSanitizer;

// ─── Test helpers ────────────────────────────────────────────────────────────

fn ollama_config(server_url: &str) -> Config {
    Config {
        provider: Provider::Ollama,
        model: "qwen3:4b".into(),
        ollama_host: server_url.to_string(),
        timeout_secs: 5,
        temperature: 0.3,
        num_predict: 256,
        ..Config::default()
    }
}

fn openai_config(server_url: &str) -> Config {
    Config {
        provider: Provider::OpenAI,
        model: "gpt-4o-mini".into(),
        openai_base_url: Some(server_url.to_string()),
        api_key: Some("test-key".into()),
        timeout_secs: 5,
        temperature: 0.3,
        num_predict: 256,
        ..Config::default()
    }
}

fn anthropic_config(server_url: &str) -> Config {
    Config {
        provider: Provider::Anthropic,
        model: "claude-sonnet-4-20250514".into(),
        anthropic_base_url: Some(format!("{server_url}/v1")),
        api_key: Some("test-key".into()),
        timeout_secs: 5,
        temperature: 0.3,
        num_predict: 256,
        ..Config::default()
    }
}

fn default_format() -> CommitFormat {
    CommitFormat::default()
}

/// Drain the token receiver and return all collected tokens.
async fn drain_tokens(mut rx: mpsc::Receiver<String>) -> Vec<String> {
    let mut tokens = Vec::new();
    while let Some(tok) = rx.recv().await {
        tokens.push(tok);
    }
    tokens
}

// ─── Ollama health check ─────────────────────────────────────────────────────

#[tokio::test]
async fn ollama_health_check_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "models": [
                {"name": "qwen3:4b"},
                {"name": "llama3:8b"}
            ]
        })))
        .mount(&server)
        .await;

    let provider = OllamaProvider::new(&ollama_config(&server.uri()));
    let models = provider.health_check().await.unwrap();

    assert_eq!(models.len(), 2);
    assert!(models.contains(&"qwen3:4b".to_string()));
    assert!(models.contains(&"llama3:8b".to_string()));
}

#[tokio::test]
async fn ollama_health_check_connection_refused() {
    // Use a port that is almost certainly not listening
    let provider = OllamaProvider::new(&ollama_config("http://127.0.0.1:1"));
    let result = provider.health_check().await;

    assert!(result.is_err(), "expected error for connection refused");
    let err = result.unwrap_err();
    assert!(
        matches!(err, Error::OllamaNotRunning { .. }),
        "expected OllamaNotRunning, got: {err:?}"
    );
}

// ─── Ollama model verification ───────────────────────────────────────────────

#[tokio::test]
async fn ollama_model_not_found() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "models": [
                {"name": "llama3:8b"},
                {"name": "codellama:7b"}
            ]
        })))
        .mount(&server)
        .await;

    let provider = OllamaProvider::new(&ollama_config(&server.uri()));
    let result = provider.verify_model().await;

    assert!(result.is_err(), "expected error when model is not found");
    let err = result.unwrap_err();
    match err {
        Error::ModelNotFound { model, available } => {
            assert_eq!(model, "qwen3:4b");
            assert!(available.contains(&"llama3:8b".to_string()));
            assert!(available.contains(&"codellama:7b".to_string()));
        }
        other => panic!("expected ModelNotFound, got: {other:?}"),
    }
}

// ─── Ollama streaming response ───────────────────────────────────────────────

#[tokio::test]
async fn ollama_streaming_response() {
    let server = MockServer::start().await;

    // NDJSON streaming: each line is a separate JSON object
    let body = [
        r#"{"response":"feat","done":false}"#,
        r#"{"response":"(scope","done":false}"#,
        r#"{"response":"): add","done":false}"#,
        r#"{"response":" feature","done":true}"#,
    ]
    .join("\n");

    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let provider = OllamaProvider::new(&ollama_config(&server.uri()));
    let (tx, rx) = mpsc::channel(32);
    let cancel = CancellationToken::new();

    let result = provider.generate("test prompt", tx, cancel).await.unwrap();

    assert_eq!(result, "feat(scope): add feature");

    // Verify tokens were streamed
    let tokens = drain_tokens(rx).await;
    assert!(
        !tokens.is_empty(),
        "expected streaming tokens to be received"
    );
}

// ─── Ollama server error ─────────────────────────────────────────────────────

#[tokio::test]
async fn ollama_server_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let provider = OllamaProvider::new(&ollama_config(&server.uri()));
    let (tx, _rx) = mpsc::channel(32);
    let cancel = CancellationToken::new();

    let result = provider.generate("test prompt", tx, cancel).await;

    assert!(result.is_err(), "expected error for 500 response");
    let err = result.unwrap_err();
    match err {
        Error::Provider { provider, message } => {
            assert_eq!(provider, "ollama");
            assert!(
                message.contains("500"),
                "expected message to contain status code 500, got: {message}"
            );
        }
        other => panic!("expected Provider error, got: {other:?}"),
    }
}

// ─── OpenAI streaming response ───────────────────────────────────────────────

#[tokio::test]
async fn openai_streaming_response() {
    let server = MockServer::start().await;

    let body = [
        r#"data: {"choices":[{"delta":{"content":"feat"},"finish_reason":null}]}"#,
        "",
        r#"data: {"choices":[{"delta":{"content":": add test"},"finish_reason":"stop"}]}"#,
        "",
        "data: [DONE]",
        "",
    ]
    .join("\n");

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let provider = OpenAiProvider::new(&openai_config(&server.uri()));
    let (tx, rx) = mpsc::channel(32);
    let cancel = CancellationToken::new();

    let result = provider.generate("test prompt", tx, cancel).await.unwrap();

    assert_eq!(result, "feat: add test");

    let tokens = drain_tokens(rx).await;
    assert!(
        !tokens.is_empty(),
        "expected streaming tokens to be received"
    );
}

// ─── OpenAI unauthorized ─────────────────────────────────────────────────────

#[tokio::test]
async fn openai_unauthorized() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(serde_json::json!({"error": {"message": "invalid API key"}})),
        )
        .mount(&server)
        .await;

    let provider = OpenAiProvider::new(&openai_config(&server.uri()));
    let result = provider.verify_connection().await;

    assert!(result.is_err(), "expected error for 401 response");
    let err = result.unwrap_err();
    match err {
        Error::Provider { provider, message } => {
            assert_eq!(provider, "openai");
            assert!(
                message.contains("invalid API key"),
                "expected 'invalid API key' in message, got: {message}"
            );
        }
        other => panic!("expected Provider error, got: {other:?}"),
    }
}

// ─── Anthropic streaming response ─────────────────────────────────────────────

#[tokio::test]
async fn anthropic_streaming_response() {
    let server = MockServer::start().await;

    // Anthropic SSE format: "event:" line followed by "data:" line
    let body = [
        "event: content_block_delta",
        r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"feat"}}"#,
        "",
        "event: content_block_delta",
        r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":": add streaming"}}"#,
        "",
        "event: message_stop",
        r#"data: {"type":"message_stop"}"#,
        "",
    ]
    .join("\n");

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let provider = AnthropicProvider::new(&anthropic_config(&server.uri()));
    let (tx, rx) = mpsc::channel(32);
    let cancel = CancellationToken::new();

    let result = provider.generate("test prompt", tx, cancel).await.unwrap();

    assert_eq!(result, "feat: add streaming");

    let tokens = drain_tokens(rx).await;
    assert!(
        !tokens.is_empty(),
        "expected streaming tokens to be received"
    );
}

// ─── Anthropic verify connection ──────────────────────────────────────────────

#[tokio::test]
async fn anthropic_verify_missing_key() {
    let config = Config {
        provider: Provider::Anthropic,
        model: "claude-sonnet-4-20250514".into(),
        api_key: None,
        timeout_secs: 5,
        ..Config::default()
    };

    let provider = AnthropicProvider::new(&config);
    let result = provider.verify_connection().await;

    assert!(result.is_err(), "expected error for missing API key");
    let err = result.unwrap_err();
    match err {
        Error::Provider { provider, message } => {
            assert_eq!(provider, "anthropic");
            assert!(
                message.contains("API key"),
                "expected API key message, got: {message}"
            );
        }
        other => panic!("expected Provider error, got: {other:?}"),
    }
}

// ─── Anthropic HTTP error ─────────────────────────────────────────────────────

#[tokio::test]
async fn anthropic_server_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let provider = AnthropicProvider::new(&anthropic_config(&server.uri()));
    let (tx, _rx) = mpsc::channel(32);
    let cancel = CancellationToken::new();

    let result = provider.generate("test prompt", tx, cancel).await;

    assert!(result.is_err(), "expected error for 500 response");
    let err = result.unwrap_err();
    match err {
        Error::Provider { provider, message } => {
            assert_eq!(provider, "anthropic");
            assert!(
                message.contains("500"),
                "expected message to contain status code 500, got: {message}"
            );
        }
        other => panic!("expected Provider error, got: {other:?}"),
    }
}

// ─── Sanitizer: full JSON pipeline ───────────────────────────────────────────

#[test]
fn sanitizer_integration_with_llm_json() {
    let raw = r#"{"type":"feat","scope":"auth","subject":"add login endpoint","body":"Implements POST /login with JWT."}"#;

    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();

    assert_eq!(
        result,
        "feat(auth): add login endpoint\n\nImplements POST /login with JWT."
    );
}

// ─── Sanitizer: preamble stripping ───────────────────────────────────────────

#[test]
fn sanitizer_integration_with_llm_preamble() {
    // Simulate an LLM that emits a preamble before the actual commit message.
    let raw = "Suggested commit: feat(cli): add verbose flag";
    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();

    assert_eq!(result, "feat(cli): add verbose flag");
}

// ─── Sanitizer: Anthropic-style output ───────────────────────────────────────

#[test]
fn sanitizer_integration_with_anthropic_style_output() {
    // Anthropic models sometimes wrap JSON in markdown code fences
    let raw = r#"```json
{"type":"fix","scope":"parser","subject":"resolve bug in token scanner","body":"Fixes off-by-one error when scanning multi-byte characters."}
```"#;

    let result = CommitSanitizer::sanitize(raw, &default_format()).unwrap();

    assert_eq!(
        result,
        "fix(parser): resolve bug in token scanner\n\nFixes off-by-one error when scanning multi-byte characters."
    );
}
