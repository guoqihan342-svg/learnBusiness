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
}

impl AppConfig {
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let text = fs::read_to_string(path)?;
        let mut config = Self::default();
        apply_performance_overrides(&mut config.performance, &text);
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

[safety]
redact_before_external_ai = {}
dry_run_ai = {}

[performance]
context_chunks = {}
chunk_char_limit = {}
",
            self.ai.provider,
            self.ai.base_url,
            self.ai.chat_model,
            self.ai.vision_model,
            self.ai.embedding_model,
            self.safety.redact_before_external_ai,
            self.safety.dry_run_ai,
            self.performance.context_chunks,
            self.performance.chunk_char_limit
        )
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub base_url: String,
    pub chat_model: String,
    pub vision_model: String,
    pub embedding_model: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "mock".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            chat_model: "gpt-4o-mini".to_string(),
            vision_model: "gpt-4o-mini".to_string(),
            embedding_model: "text-embedding-3-small".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_contains_safety_and_performance_without_api_key() {
        let config = AppConfig::default().to_toml_string();
        assert!(config.contains("[safety]"));
        assert!(config.contains("[performance]"));
        assert!(config.contains("context_chunks = 5"));
        assert!(config.contains("chunk_char_limit = 1600"));
        assert!(!config.contains("api_key"));
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
}
