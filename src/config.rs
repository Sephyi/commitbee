// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use directories::ProjectDirs;
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

    #[serde(skip)]
    pub api_key: Option<String>,

    #[serde(default = "default_max_diff_lines")]
    pub max_diff_lines: usize,

    #[serde(default = "default_max_file_lines")]
    pub max_file_lines: usize,

    /// Maximum context characters for LLM prompt (~4 chars per token)
    /// Default 24000 is safe for 8K context models
    #[serde(default = "default_max_context_chars")]
    pub max_context_chars: usize,

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
            format: CommitFormat::default(),
        }
    }
}

impl Config {
    /// Load with priority: CLI > ENV > file > defaults
    pub fn load(cli: &Cli) -> Result<Self> {
        let mut config = Self::load_from_file()?;
        config.apply_env();
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

    fn load_from_file() -> Result<Self> {
        let Some(path) = Self::config_path() else {
            return Ok(Self::default());
        };

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(|e| Error::Config(e.to_string()))
    }

    fn apply_env(&mut self) {
        if let Ok(p) = std::env::var("COMMITBEE_PROVIDER") {
            self.provider = match p.to_lowercase().as_str() {
                "openai" => Provider::OpenAI,
                "anthropic" => Provider::Anthropic,
                _ => Provider::Ollama,
            };
        }

        if let Ok(m) = std::env::var("COMMITBEE_MODEL") {
            self.model = m;
        }

        if let Ok(h) = std::env::var("COMMITBEE_OLLAMA_HOST") {
            self.ollama_host = h;
        }

        // API key: COMMITBEE_API_KEY > provider-specific
        self.api_key = std::env::var("COMMITBEE_API_KEY")
            .or_else(|_| match self.provider {
                Provider::OpenAI => std::env::var("OPENAI_API_KEY"),
                Provider::Anthropic => std::env::var("ANTHROPIC_API_KEY"),
                Provider::Ollama => Err(std::env::VarError::NotPresent),
            })
            .ok();
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
