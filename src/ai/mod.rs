use std::net::IpAddr;
use std::path::PathBuf;
use std::{env, result};

use anyhow::{Context, Result, bail, ensure};
use reqwest::Url;

use crate::ai::http::HttpRequestHeader;
use crate::config::AiConfig;

pub mod cache;
pub mod http;
pub mod http_provider;
pub mod redaction;
pub mod runtime;

pub use http_provider::{HttpAiProvider, OpenAiCompatibleProvider};
pub use runtime::{AiRuntime, ImageDescriptionResult, estimate_tokens};

pub trait AiProvider {
    fn describe_image(&self, image: &ImageInput, prompt: &str) -> Result<ImageUnderstanding>;
    fn summarize_chunks(&self, chunks: &[AiTextChunk], prompt: &str) -> Result<Summary>;
    fn embed_texts(&self, texts: &[String]) -> Result<Embeddings>;
    fn answer(&self, question: &str, contexts: &[AiTextChunk]) -> Result<Answer>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInput {
    pub path: PathBuf,
    pub mime_type: String,
    pub content_hash: String,
}

impl ImageInput {
    pub fn new(
        path: impl Into<PathBuf>,
        mime_type: impl Into<String>,
        content_hash: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            mime_type: mime_type.into(),
            content_hash: content_hash.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiTextChunk {
    pub id: String,
    pub text: String,
}

impl AiTextChunk {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageUnderstanding {
    pub description: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub text: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Embeddings {
    pub vectors: Vec<Vec<f32>>,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub text: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockAiProvider {
    model: String,
}

impl Default for MockAiProvider {
    fn default() -> Self {
        Self {
            model: "mock-ai".to_string(),
        }
    }
}

impl MockAiProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
        }
    }
}

impl AiProvider for MockAiProvider {
    fn describe_image(&self, image: &ImageInput, prompt: &str) -> Result<ImageUnderstanding> {
        Ok(ImageUnderstanding {
            description: format!(
                "mock description for {} using prompt '{}' ({})",
                image.content_hash,
                prompt,
                image.path.display()
            ),
            model: self.model.clone(),
        })
    }

    fn summarize_chunks(&self, chunks: &[AiTextChunk], prompt: &str) -> Result<Summary> {
        let ids = chunks
            .iter()
            .map(|chunk| chunk.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        Ok(Summary {
            text: format!("mock summary for [{}] using prompt '{}'", ids, prompt),
            model: self.model.clone(),
        })
    }

    fn embed_texts(&self, texts: &[String]) -> Result<Embeddings> {
        let vectors = texts
            .iter()
            .map(|text| deterministic_embedding(text))
            .collect();
        Ok(Embeddings {
            vectors,
            model: self.model.clone(),
        })
    }

    fn answer(&self, question: &str, contexts: &[AiTextChunk]) -> Result<Answer> {
        let sources = contexts
            .iter()
            .map(|chunk| chunk.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        Ok(Answer {
            text: format!("mock answer to '{}' from [{}]", question, sources),
            model: self.model.clone(),
        })
    }
}

fn deterministic_embedding(text: &str) -> Vec<f32> {
    let byte_sum = text.bytes().map(u32::from).sum::<u32>() as f32;
    vec![text.chars().count() as f32, byte_sum]
}

impl<T: AiProvider + ?Sized> AiProvider for Box<T> {
    fn describe_image(&self, image: &ImageInput, prompt: &str) -> Result<ImageUnderstanding> {
        (**self).describe_image(image, prompt)
    }

    fn summarize_chunks(&self, chunks: &[AiTextChunk], prompt: &str) -> Result<Summary> {
        (**self).summarize_chunks(chunks, prompt)
    }

    fn embed_texts(&self, texts: &[String]) -> Result<Embeddings> {
        (**self).embed_texts(texts)
    }

    fn answer(&self, question: &str, contexts: &[AiTextChunk]) -> Result<Answer> {
        (**self).answer(question, contexts)
    }
}

pub fn api_key_from_env(config: &AiConfig) -> Option<String> {
    non_empty_string(&config.api_key_env).and_then(|name| match env::var(name) {
        result::Result::Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    })
}

pub fn headers_from_config(config: &AiConfig) -> Result<Vec<HttpRequestHeader>> {
    let mut headers = Vec::with_capacity(config.headers.len() + 1);
    let has_authorization = config
        .headers
        .keys()
        .any(|name| name.eq_ignore_ascii_case("authorization"));

    if !has_authorization && let Some(api_key_env) = non_empty_string(&config.api_key_env) {
        let api_key = env::var(&api_key_env)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .with_context(|| {
                format!("API key/header environment variable '{api_key_env}' is not set or empty")
            })?;
        headers.push(HttpRequestHeader::new(
            "Authorization",
            format!("Bearer {api_key}"),
        )?);
    }

    for (name, value_template) in &config.headers {
        let value = expand_env_placeholders(value_template)?;
        headers.push(HttpRequestHeader::new(name, value)?);
    }
    Ok(headers)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProviderKind {
    Mock,
    Http,
}

impl AiProviderKind {
    pub fn parse(provider: &str) -> Result<Self> {
        match provider.trim().to_ascii_lowercase().as_str() {
            "mock" => Ok(Self::Mock),
            "http" | "openai" | "openai-compatible" | "openai_compatible" => Ok(Self::Http),
            other => bail!("unsupported AI provider '{other}'; supported providers: mock, http"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderDescriptor {
    pub kind: AiProviderKind,
    pub provider: String,
    pub base_url: String,
    pub chat_model: String,
    pub vision_model: String,
    pub embedding_model: String,
    pub api_key_env: Option<String>,
    pub requires_api_key: bool,
    pub local_only: bool,
    pub supports_vision: bool,
    pub supports_embeddings: bool,
}

impl AiProviderDescriptor {
    pub fn from_config(config: &AiConfig) -> Result<Self> {
        let kind = AiProviderKind::parse(&config.provider)?;
        if matches!(kind, AiProviderKind::Http) {
            ensure_valid_http_base_url(&config.base_url)?;
        }

        let requires_api_key = non_empty_string(&config.api_key_env).is_some()
            && !config
                .headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("authorization"));
        Ok(Self {
            kind,
            provider: config.provider.clone(),
            base_url: config.base_url.clone(),
            chat_model: config.chat_model.clone(),
            vision_model: config.vision_model.clone(),
            embedding_model: config.embedding_model.clone(),
            api_key_env: non_empty_string(&config.api_key_env),
            requires_api_key,
            local_only: matches!(kind, AiProviderKind::Http)
                && is_loopback_base_url(&config.base_url),
            supports_vision: true,
            supports_embeddings: true,
        })
    }
}

pub fn provider_from_config(
    config: &AiConfig,
    _api_key: Option<String>,
) -> Result<Box<dyn AiProvider>> {
    let descriptor = AiProviderDescriptor::from_config(config)?;
    match descriptor.kind {
        AiProviderKind::Mock => Ok(Box::new(MockAiProvider::default())),
        AiProviderKind::Http => Ok(Box::new(HttpAiProvider::from_config(config))),
    }
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn ensure_valid_http_base_url(base_url: &str) -> Result<()> {
    let url = Url::parse(base_url.trim()).context("AI base_url must be a valid URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "AI base_url must use http or https"
    );
    ensure!(url.host_str().is_some(), "AI base_url must include a host");
    Ok(())
}

fn is_loopback_base_url(base_url: &str) -> bool {
    let Ok(url) = Url::parse(base_url.trim()) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn expand_env_placeholders(value: &str) -> Result<String> {
    let mut output = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find('}') else {
            bail!("AI header environment placeholder is missing closing brace");
        };
        let env_name = &after_start[..end];
        ensure!(
            !env_name.trim().is_empty(),
            "AI header environment placeholder is empty"
        );
        let env_value = env::var(env_name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .with_context(|| {
                format!("API key/header environment variable '{env_name}' is not set or empty")
            })?;
        output.push_str(&env_value);
        rest = &after_start[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_ai_provider_returns_deterministic_outputs() {
        let provider = MockAiProvider::default();
        let contexts = vec![AiTextChunk::new("chunk-1", "hello")];

        let first = provider.answer("what?", &contexts).unwrap();
        let second = provider.answer("what?", &contexts).unwrap();
        assert_eq!(first, second);

        let texts = vec!["hello".to_string(), "business".to_string()];
        let embeddings = provider.embed_texts(&texts).unwrap();
        assert_eq!(embeddings.vectors.len(), 2);
        assert_eq!(embeddings.vectors[0], vec![5.0, 532.0]);
    }

    #[test]
    fn http_provider_reports_missing_api_key_env_before_network() {
        unsafe {
            env::remove_var("LEARNBUSINESS_MISSING_API_KEY");
        }
        let config = AiConfig {
            provider: "http".to_string(),
            base_url: "https://api.example.test/v1".to_string(),
            chat_model: "chat".to_string(),
            vision_model: "vision".to_string(),
            embedding_model: "embedding".to_string(),
            api_key_env: "LEARNBUSINESS_MISSING_API_KEY".to_string(),
            headers: Default::default(),
        };
        let provider = HttpAiProvider::from_config(&config);

        let error = provider.answer("question", &[]).unwrap_err().to_string();
        assert!(error.contains("LEARNBUSINESS_MISSING_API_KEY"));
    }

    #[test]
    fn http_descriptor_accepts_configurable_base_url_without_local_model_semantics() {
        let config = AiConfig {
            provider: "http".to_string(),
            base_url: "http://localhost:8000/v1".to_string(),
            chat_model: "business-chat".to_string(),
            vision_model: "business-vision".to_string(),
            embedding_model: "business-embedding".to_string(),
            api_key_env: String::new(),
            headers: Default::default(),
        };

        let descriptor = AiProviderDescriptor::from_config(&config).unwrap();
        assert_eq!(descriptor.kind, AiProviderKind::Http);
        assert_eq!(descriptor.base_url, "http://localhost:8000/v1");
        assert!(descriptor.local_only);
        assert!(!descriptor.requires_api_key);
        assert!(descriptor.supports_vision);
        assert!(descriptor.supports_embeddings);

        let remote = AiConfig {
            base_url: "https://gateway.example.com/v1".to_string(),
            ..config
        };
        let descriptor = AiProviderDescriptor::from_config(&remote).unwrap();
        assert_eq!(descriptor.kind, AiProviderKind::Http);
        assert!(!descriptor.local_only);
    }

    #[test]
    fn http_headers_expand_environment_placeholders() {
        let env_name = "LEARNBUSINESS_HEADER_TEST_KEY";
        unsafe {
            env::set_var(env_name, "secret-from-env");
        }
        let mut headers = std::collections::BTreeMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer ${LEARNBUSINESS_HEADER_TEST_KEY}".to_string(),
        );
        headers.insert("X-App".to_string(), "learnBusiness".to_string());
        let config = AiConfig {
            provider: "http".to_string(),
            base_url: "http://localhost:8000/v1".to_string(),
            chat_model: "chat".to_string(),
            vision_model: "vision".to_string(),
            embedding_model: "embedding".to_string(),
            api_key_env: String::new(),
            headers,
        };

        let resolved = headers_from_config(&config).unwrap();
        assert_eq!(resolved[0].name(), "authorization");
        assert_eq!(resolved[0].value(), "Bearer secret-from-env");
        assert_eq!(resolved[1].name(), "x-app");
        assert_eq!(resolved[1].value(), "learnBusiness");
        unsafe {
            env::remove_var(env_name);
        }
    }

    #[test]
    fn http_headers_fail_before_network_when_environment_is_missing() {
        unsafe {
            env::remove_var("LEARNBUSINESS_MISSING_HEADER_KEY");
        }
        let mut headers = std::collections::BTreeMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer ${LEARNBUSINESS_MISSING_HEADER_KEY}".to_string(),
        );
        let config = AiConfig {
            provider: "http".to_string(),
            base_url: "https://gateway.example.com/v1".to_string(),
            chat_model: "chat".to_string(),
            vision_model: "vision".to_string(),
            embedding_model: "embedding".to_string(),
            api_key_env: String::new(),
            headers,
        };

        let error = headers_from_config(&config).unwrap_err().to_string();
        assert!(error.contains("LEARNBUSINESS_MISSING_HEADER_KEY"));
    }

    #[test]
    fn descriptor_covers_supported_provider_matrix() {
        let mock = AiConfig {
            provider: "mock".to_string(),
            base_url: "http://localhost:8000/v1".to_string(),
            chat_model: "chat".to_string(),
            vision_model: "vision".to_string(),
            embedding_model: "embedding".to_string(),
            api_key_env: String::new(),
            headers: Default::default(),
        };
        let descriptor = AiProviderDescriptor::from_config(&mock).unwrap();
        assert_eq!(descriptor.kind, AiProviderKind::Mock);
        assert!(!descriptor.local_only);
        assert!(!descriptor.requires_api_key);

        let http = AiConfig {
            provider: "http".to_string(),
            base_url: "https://gateway.example.com/v1".to_string(),
            chat_model: "gpt-4o-mini".to_string(),
            vision_model: "gpt-4o-mini".to_string(),
            embedding_model: "text-embedding-3-small".to_string(),
            api_key_env: "LEARNBUSINESS_API_KEY".to_string(),
            headers: Default::default(),
        };
        let descriptor = AiProviderDescriptor::from_config(&http).unwrap();
        assert_eq!(descriptor.kind, AiProviderKind::Http);
        assert!(!descriptor.local_only);
        assert!(descriptor.requires_api_key);
        assert_eq!(
            descriptor.api_key_env.as_deref(),
            Some("LEARNBUSINESS_API_KEY")
        );

        let legacy_alias = AiConfig {
            provider: "openai-compatible".to_string(),
            api_key_env: String::new(),
            ..http
        };
        let descriptor = AiProviderDescriptor::from_config(&legacy_alias).unwrap();
        assert_eq!(descriptor.kind, AiProviderKind::Http);
    }

    #[test]
    fn descriptor_rejects_unknown_provider_with_supported_names() {
        let config = AiConfig {
            provider: "remote-model".to_string(),
            base_url: "https://model.example.com/v1".to_string(),
            chat_model: "chat".to_string(),
            vision_model: "vision".to_string(),
            embedding_model: "embedding".to_string(),
            api_key_env: String::new(),
            headers: Default::default(),
        };

        let error = AiProviderDescriptor::from_config(&config)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unsupported AI provider"));
        assert!(error.contains("mock"));
        assert!(error.contains("http"));
    }

    #[test]
    fn http_descriptor_rejects_invalid_base_url_scheme() {
        let config = AiConfig {
            provider: "http".to_string(),
            base_url: "file:///tmp/model".to_string(),
            chat_model: "chat".to_string(),
            vision_model: "vision".to_string(),
            embedding_model: "embedding".to_string(),
            api_key_env: String::new(),
            headers: Default::default(),
        };

        let error = AiProviderDescriptor::from_config(&config)
            .unwrap_err()
            .to_string();
        assert!(error.contains("http or https"));
    }

    #[test]
    fn api_key_from_env_reads_named_environment_variable_only() {
        let env_name = "LEARNBUSINESS_PROVIDER_TEST_KEY";
        unsafe {
            env::set_var(env_name, "secret-from-env");
        }
        let config = AiConfig {
            provider: "http".to_string(),
            base_url: "https://gateway.example.com/v1".to_string(),
            chat_model: "chat".to_string(),
            vision_model: "vision".to_string(),
            embedding_model: "embedding".to_string(),
            api_key_env: env_name.to_string(),
            headers: Default::default(),
        };

        assert_eq!(
            api_key_from_env(&config).as_deref(),
            Some("secret-from-env")
        );
        unsafe {
            env::remove_var(env_name);
        }
    }
}
