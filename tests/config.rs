// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use commitbee::config::{Config, Provider};

// ─── Default values ──────────────────────────────────────────────────────────

#[test]
fn default_config_values() {
    let config = Config::default();
    assert_eq!(config.provider, Provider::Ollama);
    assert_eq!(config.model, "qwen3:4b");
    assert_eq!(config.ollama_host, "http://localhost:11434");
    assert!(config.api_key.is_none());
    assert_eq!(config.max_diff_lines, 500);
    assert_eq!(config.max_file_lines, 100);
    assert_eq!(config.max_context_chars, 24000);
    assert_eq!(config.timeout_secs, 300);
    assert!((config.temperature - 0.3).abs() < f32::EPSILON);
    assert_eq!(config.num_predict, 256);
    assert!(config.format.include_body);
    assert!(config.format.include_scope);
    assert!(config.format.lowercase_subject);
}

// ─── TOML deserialization ────────────────────────────────────────────────────

#[test]
fn load_from_valid_toml() {
    let toml_str = r#"
provider = "openai"
model = "gpt-4o"
ollama_host = "http://localhost:11434"
max_diff_lines = 300
max_file_lines = 50
max_context_chars = 16000

[format]
include_body = false
include_scope = true
lowercase_subject = false
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.provider, Provider::OpenAI);
    assert_eq!(config.model, "gpt-4o");
    assert_eq!(config.max_diff_lines, 300);
    assert_eq!(config.max_file_lines, 50);
    assert_eq!(config.max_context_chars, 16000);
    assert!(!config.format.include_body);
    assert!(config.format.include_scope);
    assert!(!config.format.lowercase_subject);
}

#[test]
fn load_partial_toml_uses_defaults() {
    let toml_str = r#"model = "llama3:8b""#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.model, "llama3:8b");
    // Everything else should be default
    assert_eq!(config.provider, Provider::Ollama);
    assert_eq!(config.ollama_host, "http://localhost:11434");
    assert_eq!(config.max_diff_lines, 500);
    assert!(config.format.include_body);
}

#[test]
fn empty_toml_uses_all_defaults() {
    let config: Config = toml::from_str("").unwrap();
    let default = Config::default();
    assert_eq!(config.provider, default.provider);
    assert_eq!(config.model, default.model);
    assert_eq!(config.max_diff_lines, default.max_diff_lines);
}

// ─── Provider display ────────────────────────────────────────────────────────

#[test]
fn provider_display_format() {
    assert_eq!(format!("{}", Provider::Ollama), "ollama");
    assert_eq!(format!("{}", Provider::OpenAI), "openai");
    assert_eq!(format!("{}", Provider::Anthropic), "anthropic");
}

// ─── Format section defaults ─────────────────────────────────────────────────

#[test]
fn format_section_defaults() {
    let toml_str = r#"provider = "ollama""#;
    let config: Config = toml::from_str(toml_str).unwrap();
    // format section missing -> all defaults
    assert!(config.format.include_body);
    assert!(config.format.include_scope);
    assert!(config.format.lowercase_subject);
}

// ─── Error handling ──────────────────────────────────────────────────────────

#[test]
fn invalid_toml_returns_error() {
    let result: std::result::Result<Config, _> = toml::from_str("provider = [invalid");
    assert!(result.is_err(), "invalid TOML should return an error");
}
