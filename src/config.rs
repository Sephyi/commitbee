// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use directories::ProjectDirs;
use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::cli::Cli;
use crate::error::{Error, Result};

/// Commit message format configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitFormat {
    /// Include body in commit message (default: true)
    #[serde(default = "default_true")]
    pub include_body: bool,

    /// Include scope in commit type, e.g., feat(scope): (default: true)
    #[serde(default = "default_true")]
    pub include_scope: bool,

    /// Enforce lowercase first character of subject (default: true)
    #[serde(default = "default_true")]
    pub lowercase_subject: bool,
}

impl Default for CommitFormat {
    fn default() -> Self {
        Self {
            include_body: true,
            include_scope: true,
            lowercase_subject: true,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Ollama,
    OpenAI,
    Anthropic,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ollama => write!(f, "ollama"),
            Self::OpenAI => write!(f, "openai"),
            Self::Anthropic => write!(f, "anthropic"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub provider: Provider,

    #[serde(default = "default_model")]
    pub model: String,

    #[serde(default = "default_ollama_host")]
    pub ollama_host: String,

    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default = "default_max_diff_lines")]
    pub max_diff_lines: usize,

    #[serde(default = "default_max_file_lines")]
    pub max_file_lines: usize,

    /// Maximum context characters for LLM prompt (~4 chars per token)
    /// Default 24000 is safe for 8K context models
    #[serde(default = "default_max_context_chars")]
    pub max_context_chars: usize,

    /// Request timeout in seconds (default 300)
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// LLM temperature (0.0-2.0, default 0.3)
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Maximum tokens to generate (default 256)
    #[serde(default = "default_num_predict")]
    pub num_predict: u32,

    /// Base URL for OpenAI-compatible APIs (default: https://api.openai.com/v1)
    #[serde(default)]
    pub openai_base_url: Option<String>,

    /// Commit message format options
    #[serde(default)]
    pub format: CommitFormat,
}

fn default_max_context_chars() -> usize {
    24_000
}

fn default_model() -> String {
    "qwen3:4b".into()
}
fn default_ollama_host() -> String {
    "http://localhost:11434".into()
}
fn default_max_diff_lines() -> usize {
    500
}
fn default_max_file_lines() -> usize {
    100
}
fn default_timeout_secs() -> u64 {
    300
}
fn default_temperature() -> f32 {
    0.3
}
fn default_num_predict() -> u32 {
    256
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: Provider::default(),
            model: default_model(),
            ollama_host: default_ollama_host(),
            api_key: None,
            max_diff_lines: default_max_diff_lines(),
            max_file_lines: default_max_file_lines(),
            max_context_chars: default_max_context_chars(),
            timeout_secs: default_timeout_secs(),
            temperature: default_temperature(),
            num_predict: default_num_predict(),
            openai_base_url: None,
            format: CommitFormat::default(),
        }
    }
}

impl Config {
    /// Load with priority: CLI > ENV > user config > project config > defaults
    pub fn load(cli: &Cli) -> Result<Self> {
        let mut figment = Figment::new().merge(Serialized::defaults(Config::default()));

        // Project-level config (.commitbee.toml in repo root)
        if let Ok(cwd) = std::env::current_dir() {
            let project_config = cwd.join(".commitbee.toml");
            if project_config.exists() {
                figment = figment.merge(Toml::file(&project_config));
            }
        }

        // User-level config
        if let Some(path) = Self::config_path() {
            if path.exists() {
                figment = figment.merge(Toml::file(&path));
            }
        }

        // Environment variables (COMMITBEE_MODEL, COMMITBEE_PROVIDER, etc.)
        // Use __ separator for nested keys (e.g., COMMITBEE_FORMAT__INCLUDE_BODY)
        figment = figment.merge(Env::prefixed("COMMITBEE_").split("__"));

        let mut config: Config = figment
            .extract()
            .map_err(|e| Error::Config(e.to_string()))?;

        // Provider-specific API key fallback
        if config.api_key.is_none() {
            config.api_key = match config.provider {
                Provider::OpenAI => std::env::var("OPENAI_API_KEY").ok(),
                Provider::Anthropic => std::env::var("ANTHROPIC_API_KEY").ok(),
                Provider::Ollama => None,
            };
        }

        // Keyring fallback (if still no key and secure-storage feature is enabled)
        #[cfg(feature = "secure-storage")]
        if config.api_key.is_none() && config.provider != Provider::Ollama {
            let provider_name = config.provider.to_string();
            if let Ok(entry) = keyring::Entry::new("commitbee", &provider_name) {
                if let Ok(key) = entry.get_password() {
                    config.api_key = Some(key);
                }
            }
        }

        // CLI overrides (highest priority)
        config.apply_cli(cli);
        config.validate()?;
        Ok(config)
    }

    pub fn config_dir() -> Option<PathBuf> {
        ProjectDirs::from("", "", "commitbee").map(|dirs| dirs.config_dir().to_path_buf())
    }

    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("config.toml"))
    }

    fn apply_cli(&mut self, cli: &Cli) {
        if let Some(ref p) = cli.provider {
            self.provider = match p.to_lowercase().as_str() {
                "openai" => Provider::OpenAI,
                "anthropic" => Provider::Anthropic,
                _ => Provider::Ollama,
            };
        }
        if let Some(ref m) = cli.model {
            self.model = m.clone();
        }
    }

    fn validate(&self) -> Result<()> {
        if self.provider != Provider::Ollama && self.api_key.is_none() {
            return Err(Error::Config(format!(
                "{} requires an API key. Set COMMITBEE_API_KEY or {}_API_KEY",
                self.provider,
                format!("{:?}", self.provider).to_uppercase()
            )));
        }

        if !(10..=10_000).contains(&self.max_diff_lines) {
            return Err(Error::Config(format!(
                "max_diff_lines must be 10–10000, got {}",
                self.max_diff_lines
            )));
        }

        if !(10..=1_000).contains(&self.max_file_lines) {
            return Err(Error::Config(format!(
                "max_file_lines must be 10–1000, got {}",
                self.max_file_lines
            )));
        }

        if !(1_000..=200_000).contains(&self.max_context_chars) {
            return Err(Error::Config(format!(
                "max_context_chars must be 1000–200000, got {}",
                self.max_context_chars
            )));
        }

        if !(1..=3600).contains(&self.timeout_secs) {
            return Err(Error::Config(format!(
                "timeout_secs must be 1–3600, got {}",
                self.timeout_secs
            )));
        }

        if !(0.0..=2.0).contains(&self.temperature) {
            return Err(Error::Config(format!(
                "temperature must be 0.0–2.0, got {}",
                self.temperature
            )));
        }

        if self.ollama_host.is_empty() {
            return Err(Error::Config("ollama_host cannot be empty".into()));
        }

        if !self.ollama_host.starts_with("http://") && !self.ollama_host.starts_with("https://") {
            return Err(Error::Config(format!(
                "ollama_host must start with http:// or https://, got '{}'",
                self.ollama_host
            )));
        }

        Ok(())
    }

    /// Create default config file with secure permissions
    pub fn create_default() -> Result<PathBuf> {
        let Some(dir) = Self::config_dir() else {
            return Err(Error::Config("Cannot determine config directory".into()));
        };

        fs::create_dir_all(&dir)?;

        let path = dir.join("config.toml");
        let content = r#"# CommitBee Configuration

# LLM provider: ollama, openai, anthropic
provider = "ollama"

# Model name (for Ollama, use `ollama list` to see available)
model = "qwen3:4b"

# Ollama server URL
ollama_host = "http://localhost:11434"

# Maximum lines of diff to include in prompt
max_diff_lines = 500

# Maximum lines per file in diff
max_file_lines = 100

# Maximum context characters for LLM prompt (~4 chars per token)
# Increase for larger models (e.g., 48000 for 16K context)
# max_context_chars = 24000

# Commit message format options
[format]
# Include body/description in commit message
include_body = true

# Include scope in commit type, e.g., feat(scope): subject
include_scope = true

# Enforce lowercase first character of subject (conventional commits best practice)
lowercase_subject = true
"#;

        fs::write(&path, content)?;

        // Set secure permissions (0600)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(path)
    }
}
