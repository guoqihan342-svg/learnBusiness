use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::config::AiConfig;

pub mod cache;
pub mod redaction;

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

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    pub base_url: String,
    pub api_key: Option<String>,
    pub chat_model: String,
    pub vision_model: String,
    pub embedding_model: String,
    client: reqwest::blocking::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        chat_model: impl Into<String>,
        vision_model: impl Into<String>,
        embedding_model: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            api_key,
            chat_model: chat_model.into(),
            vision_model: vision_model.into(),
            embedding_model: embedding_model.into(),
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn from_config(config: &AiConfig, api_key: Option<String>) -> Self {
        Self::new(
            config.base_url.clone(),
            api_key,
            config.chat_model.clone(),
            config.vision_model.clone(),
            config.embedding_model.clone(),
        )
    }

    fn api_key(&self) -> Result<&str> {
        self.api_key.as_deref().filter(|key| !key.is_empty()).ok_or_else(|| {
            anyhow::anyhow!(
                "OpenAI-compatible provider requires an API key; configure api_key or set a supported API key environment variable"
            )
        })
    }

    fn not_implemented(&self, operation: &str) -> Result<()> {
        let _ = self.api_key()?;
        let _client = self.client.clone();
        bail!(
            "OpenAI-compatible provider '{}' call skeleton is configured but HTTP execution is not implemented yet",
            operation
        )
    }
}

impl AiProvider for OpenAiCompatibleProvider {
    fn describe_image(&self, _image: &ImageInput, _prompt: &str) -> Result<ImageUnderstanding> {
        self.not_implemented("describe_image")?;
        unreachable!()
    }

    fn summarize_chunks(&self, _chunks: &[AiTextChunk], _prompt: &str) -> Result<Summary> {
        self.not_implemented("summarize_chunks")?;
        unreachable!()
    }

    fn embed_texts(&self, _texts: &[String]) -> Result<Embeddings> {
        self.not_implemented("embed_texts")?;
        unreachable!()
    }

    fn answer(&self, _question: &str, _contexts: &[AiTextChunk]) -> Result<Answer> {
        self.not_implemented("answer")?;
        unreachable!()
    }
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
    fn openai_provider_requires_api_key_before_network() {
        let provider = OpenAiCompatibleProvider::new(
            "https://api.example.test/v1",
            None,
            "chat",
            "vision",
            "embedding",
        );

        let error = provider.answer("question", &[]).unwrap_err().to_string();
        assert!(error.contains("requires an API key"));
    }
}
