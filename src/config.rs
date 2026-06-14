use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub ai: AiConfig,
    pub safety: SafetyConfig,
}

impl AppConfig {
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
",
            self.ai.provider,
            self.ai.base_url,
            self.ai.chat_model,
            self.ai.vision_model,
            self.ai.embedding_model,
            self.safety.redact_before_external_ai,
            self.safety.dry_run_ai
        )
    }
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
