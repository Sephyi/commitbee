// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::path::PathBuf;

use clap::Parser;
use commitbee::cli::Cli;
use commitbee::config::{Config, Provider};

// ─── Default values ──────────────────────────────────────────────────────────

#[test]
fn default_config_values() {
    let config = Config::default();
    assert_eq!(config.provider, Provider::Ollama);
    assert_eq!(config.model, "qwen3.5:4b");
    assert_eq!(config.ollama_host, "http://localhost:11434");
    assert!(config.api_key.is_none());
    assert_eq!(config.max_diff_lines, 500);
    assert_eq!(config.max_file_lines, 100);
    assert_eq!(config.max_context_chars, 24000);
    assert_eq!(config.timeout_secs, 300);
    assert!((config.temperature - 0.3).abs() < f32::EPSILON);
    assert_eq!(config.num_predict, 256);
    assert!(!config.think);
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

#[test]
fn think_defaults_to_false() {
    let config: Config = toml::from_str("").unwrap();
    assert!(!config.think);
}

#[test]
fn think_can_be_enabled() {
    let config: Config = toml::from_str("think = true").unwrap();
    assert!(config.think);
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

// ─── Exclude patterns ─────────────────────────────────────────────────────────

#[test]
fn exclude_patterns_default_empty() {
    let config = Config::default();
    assert!(config.exclude_patterns.is_empty());
}

#[test]
fn exclude_patterns_from_toml() {
    let toml_str = r#"
exclude_patterns = ["*.lock", "**/*.generated.*"]
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.exclude_patterns.len(), 2);
    assert_eq!(config.exclude_patterns[0], "*.lock");
    assert_eq!(config.exclude_patterns[1], "**/*.generated.*");
}

#[test]
fn exclude_patterns_empty_list_from_toml() {
    let toml_str = r#"
exclude_patterns = []
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert!(config.exclude_patterns.is_empty());
}

// ─── Exclude pattern glob matching ────────────────────────────────────────────

#[test]
fn exclude_glob_matches_lock_files() {
    use globset::{Glob, GlobSetBuilder};

    let patterns = vec!["*.lock".to_string()];
    let mut builder = GlobSetBuilder::new();
    for p in &patterns {
        builder.add(Glob::new(p).unwrap());
    }
    let set = builder.build().unwrap();

    assert!(set.is_match(PathBuf::from("Cargo.lock")));
    assert!(set.is_match(PathBuf::from("yarn.lock")));
    assert!(!set.is_match(PathBuf::from("package-lock.json")));
    assert!(!set.is_match(PathBuf::from("src/main.rs")));
}

#[test]
fn exclude_glob_matches_nested_generated() {
    use globset::{Glob, GlobSetBuilder};

    let patterns = vec!["**/*.generated.*".to_string()];
    let mut builder = GlobSetBuilder::new();
    for p in &patterns {
        builder.add(Glob::new(p).unwrap());
    }
    let set = builder.build().unwrap();

    assert!(set.is_match(PathBuf::from("src/schema.generated.rs")));
    assert!(set.is_match(PathBuf::from("deep/nested/file.generated.ts")));
    assert!(!set.is_match(PathBuf::from("src/main.rs")));
}

#[test]
fn exclude_glob_multiple_patterns() {
    use globset::{Glob, GlobSetBuilder};

    let patterns = vec!["*.lock".to_string(), "*.min.js".to_string()];
    let mut builder = GlobSetBuilder::new();
    for p in &patterns {
        builder.add(Glob::new(p).unwrap());
    }
    let set = builder.build().unwrap();

    assert!(set.is_match(PathBuf::from("Cargo.lock")));
    assert!(set.is_match(PathBuf::from("bundle.min.js")));
    assert!(!set.is_match(PathBuf::from("src/app.js")));
}

#[test]
fn exclude_glob_directory_pattern() {
    use globset::{Glob, GlobSetBuilder};

    let patterns = vec!["vendor/**".to_string()];
    let mut builder = GlobSetBuilder::new();
    for p in &patterns {
        builder.add(Glob::new(p).unwrap());
    }
    let set = builder.build().unwrap();

    assert!(set.is_match(PathBuf::from("vendor/lib.rs")));
    assert!(set.is_match(PathBuf::from("vendor/deep/nested.rs")));
    assert!(!set.is_match(PathBuf::from("src/vendor.rs")));
}

// ─── CLI flag parsing ─────────────────────────────────────────────────────────

#[test]
fn cli_exclude_single() {
    let cli = Cli::try_parse_from(["commitbee", "--exclude", "*.lock"]).unwrap();
    assert_eq!(cli.exclude, vec!["*.lock"]);
}

#[test]
fn cli_exclude_multiple() {
    let cli =
        Cli::try_parse_from(["commitbee", "--exclude", "*.lock", "--exclude", "*.min.js"]).unwrap();
    assert_eq!(cli.exclude, vec!["*.lock", "*.min.js"]);
}

#[test]
fn cli_exclude_empty_by_default() {
    let cli = Cli::try_parse_from(["commitbee"]).unwrap();
    assert!(cli.exclude.is_empty());
}

#[test]
fn cli_clipboard_flag() {
    let cli = Cli::try_parse_from(["commitbee", "--clipboard"]).unwrap();
    assert!(cli.clipboard);
}

#[test]
fn cli_clipboard_default_false() {
    let cli = Cli::try_parse_from(["commitbee"]).unwrap();
    assert!(!cli.clipboard);
}

#[test]
fn cli_clipboard_with_dry_run() {
    let cli = Cli::try_parse_from(["commitbee", "--clipboard", "--dry-run"]).unwrap();
    assert!(cli.clipboard);
    assert!(cli.dry_run);
}

// ─── Error handling ──────────────────────────────────────────────────────────

#[test]
fn invalid_toml_returns_error() {
    let result: std::result::Result<Config, _> = toml::from_str("provider = [invalid");
    assert!(result.is_err(), "invalid TOML should return an error");
}

// ─── Generated config sync ──────────────────────────────────────────────────

#[test]
fn generated_config_round_trips_to_defaults() {
    let generated = Config::generate_default_config();

    // Strip comment-only lines and commented-out key=value lines,
    // keep only active key=value lines (plus section headers).
    let active_toml: String = generated
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.is_empty()
                || trimmed.starts_with('[')
                || (!trimmed.starts_with('#') && trimmed.contains('='))
        })
        .map(|l| format!("{l}\n"))
        .collect();

    let parsed: Config = toml::from_str(&active_toml)
        .expect("generated config with active lines should parse as valid TOML");

    let default = Config::default();
    assert_eq!(parsed.provider, default.provider);
    assert_eq!(parsed.model, default.model);
    assert_eq!(parsed.ollama_host, default.ollama_host);
    assert_eq!(parsed.max_diff_lines, default.max_diff_lines);
    assert_eq!(parsed.max_file_lines, default.max_file_lines);
    assert_eq!(parsed.max_context_chars, default.max_context_chars);
    assert_eq!(parsed.timeout_secs, default.timeout_secs);
    assert!((parsed.temperature - default.temperature).abs() < f32::EPSILON);
    assert_eq!(parsed.num_predict, default.num_predict);
    assert_eq!(parsed.think, default.think);
    assert_eq!(parsed.rename_threshold, default.rename_threshold);
    assert_eq!(parsed.learn_from_history, default.learn_from_history);
    assert_eq!(parsed.history_sample_size, default.history_sample_size);
    assert!(parsed.format.include_body);
    assert!(parsed.format.include_scope);
    assert!(parsed.format.lowercase_subject);
}

#[test]
fn generated_config_includes_all_active_fields() {
    let generated = Config::generate_default_config();

    // All uncommented (active) fields must appear
    let active_keys = [
        "provider",
        "model",
        "ollama_host",
        "max_diff_lines",
        "max_file_lines",
    ];
    for key in active_keys {
        assert!(
            generated.lines().any(|l| {
                let trimmed = l.trim();
                !trimmed.starts_with('#') && trimmed.starts_with(key)
            }),
            "active field '{key}' missing from generated config"
        );
    }

    // All commented-out fields must appear (as # key = value)
    let commented_keys = [
        "timeout_secs",
        "temperature",
        "num_predict",
        "think",
        "max_context_chars",
        "rename_threshold",
        "locale",
        "learn_from_history",
        "history_sample_size",
        "openai_base_url",
        "anthropic_base_url",
        "system_prompt_path",
        "template_path",
        "custom_secret_patterns",
        "disabled_secret_patterns",
        "exclude_patterns",
    ];
    for key in commented_keys {
        assert!(
            generated
                .lines()
                .any(|l| l.starts_with("# ") && l[2..].trim_start().starts_with(key)),
            "commented field '{key}' missing from generated config"
        );
    }
}
