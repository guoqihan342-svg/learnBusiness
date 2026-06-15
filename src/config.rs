use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "learnBusiness";
pub const WORKSPACE_DIR_NAME: &str = ".learnBusiness";
pub const CONFIG_DIR_NAME: &str = "config";
pub const APP_CONFIG_FILE_NAME: &str = "app.toml";
pub const DEFAULT_CONTEXT_CHUNKS: usize = 5;
pub const MAX_CONTEXT_CHUNKS: usize = 20;
pub const DEFAULT_CHUNK_CHAR_LIMIT: usize = 1600;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub ai: AiConfig,
    pub safety: SafetyConfig,
    pub performance: PerformanceConfig,
    pub logging: LoggingConfig,
}

impl AppConfig {
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let text = fs::read_to_string(path)?;
        let mut config = Self::default();
        apply_ai_overrides(&mut config, &text);
        apply_performance_overrides(&mut config.performance, &text);
        apply_logging_overrides(&mut config.logging, &text);
        Ok(config)
    }

    pub fn to_toml_string(&self) -> String {
        format!(
            "\
[ai]
provider = \"{}\"
base_url = \"{}\"
chat_model = \"{}\"
vision_model = \"{}\"
embedding_model = \"{}\"
api_key_env = \"{}\"

[safety]
redact_before_external_ai = {}
dry_run_ai = {}

[performance]
context_chunks = {}
chunk_char_limit = {}

[logging]
trace_enabled = {}
",
            self.ai.provider,
            self.ai.base_url,
            self.ai.chat_model,
            self.ai.vision_model,
            self.ai.embedding_model,
            self.ai.api_key_env,
            self.safety.redact_before_external_ai,
            self.safety.dry_run_ai,
            self.performance.context_chunks,
            self.performance.chunk_char_limit,
            self.logging.trace_enabled
        )
    }
}

fn apply_ai_overrides(config: &mut AppConfig, text: &str) {
    let mut section = "";
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(&['[', ']'][..]).trim();
            continue;
        }
        if section != "ai" {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let parsed = parse_string_value(value);
        match key.trim() {
            "provider" => {
                config.ai.provider = parsed;
            }
            "base_url" => {
                config.ai.base_url = parsed;
            }
            "chat_model" => {
                config.ai.chat_model = parsed;
            }
            "vision_model" => {
                config.ai.vision_model = parsed;
            }
            "embedding_model" => {
                config.ai.embedding_model = parsed;
            }
            "api_key_env" => {
                config.ai.api_key_env = parsed;
            }
            _ => {}
        }
    }
}

fn apply_performance_overrides(performance: &mut PerformanceConfig, text: &str) {
    let mut section = "";
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(&['[', ']'][..]).trim();
            continue;
        }
        if section != "performance" {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(parsed) = parse_usize_value(value) else {
            continue;
        };
        match key.trim() {
            "context_chunks" => {
                performance.context_chunks = parsed.clamp(1, MAX_CONTEXT_CHUNKS);
            }
            "chunk_char_limit" => {
                performance.chunk_char_limit = parsed.max(1);
            }
            _ => {}
        }
    }
}

fn apply_logging_overrides(logging: &mut LoggingConfig, text: &str) {
    let mut section = "";
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(&['[', ']'][..]).trim();
            continue;
        }
        if section != "logging" {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "trace_enabled" {
            logging.trace_enabled = parse_bool_value(value).unwrap_or(logging.trace_enabled);
        }
    }
}

fn parse_usize_value(value: &str) -> Option<usize> {
    value
        .split('#')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .parse::<usize>()
        .ok()
}

fn parse_string_value(value: &str) -> String {
    value
        .split('#')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .to_string()
}

fn parse_bool_value(value: &str) -> Option<bool> {
    match value
        .split('#')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .to_ascii_lowercase()
        .as_str()
    {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub base_url: String,
    pub chat_model: String,
    pub vision_model: String,
    pub embedding_model: String,
    pub api_key_env: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "mock".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            chat_model: "gpt-4o-mini".to_string(),
            vision_model: "gpt-4o-mini".to_string(),
            embedding_model: "text-embedding-3-small".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyConfig {
    pub redact_before_external_ai: bool,
    pub dry_run_ai: bool,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            redact_before_external_ai: true,
            dry_run_ai: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub context_chunks: usize,
    pub chunk_char_limit: usize,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            context_chunks: DEFAULT_CONTEXT_CHUNKS,
            chunk_char_limit: DEFAULT_CHUNK_CHAR_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub trace_enabled: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            trace_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_contains_safety_and_performance_without_api_key() {
        let config = AppConfig::default().to_toml_string();
        assert!(config.contains("[safety]"));
        assert!(config.contains("[performance]"));
        assert!(config.contains("[logging]"));
        assert!(config.contains("context_chunks = 5"));
        assert!(config.contains("chunk_char_limit = 1600"));
        assert!(config.contains("trace_enabled = true"));
        assert!(config.contains("api_key_env = \"OPENAI_API_KEY\""));
        assert!(!config.contains("api_key ="));
    }

    #[test]
    fn loads_context_chunks_from_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.toml");
        std::fs::write(
            &path,
            "\
[performance]
context_chunks = 2
chunk_char_limit = 1200
",
        )
        .unwrap();

        let config = AppConfig::load_or_default(&path).unwrap();
        assert_eq!(config.performance.context_chunks, 2);
        assert_eq!(config.performance.chunk_char_limit, 1200);
    }

    #[test]
    fn loads_local_ai_provider_from_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.toml");
        std::fs::write(
            &path,
            "\
[ai]
provider = \"ollama\"
base_url = \"http://127.0.0.1:11434\"
chat_model = \"qwen2.5\"
vision_model = \"llava\"
embedding_model = \"nomic-embed-text\"
api_key_env = \"\"
",
        )
        .unwrap();

        let config = AppConfig::load_or_default(&path).unwrap();
        assert_eq!(config.ai.provider, "ollama");
        assert_eq!(config.ai.base_url, "http://127.0.0.1:11434");
        assert_eq!(config.ai.chat_model, "qwen2.5");
        assert_eq!(config.ai.vision_model, "llava");
        assert_eq!(config.ai.embedding_model, "nomic-embed-text");
        assert_eq!(config.ai.api_key_env, "");
    }

    #[test]
    fn loads_logging_config_from_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.toml");
        std::fs::write(
            &path,
            "\
[logging]
trace_enabled = false
",
        )
        .unwrap();

        let config = AppConfig::load_or_default(&path).unwrap();
        assert!(!config.logging.trace_enabled);
    }
}
