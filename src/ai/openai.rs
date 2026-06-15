use anyhow::{Context, Result};
use base64::Engine;
use serde_json::{Value, json};

use crate::ai::http::{join_url, post_json};
use crate::ai::{
    AiProvider, AiTextChunk, Answer, Embeddings, ImageInput, ImageUnderstanding, Summary,
};
use crate::config::AiConfig;

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
                "OpenAI-compatible provider requires an API key; configure api_key_env to point at an API key environment variable"
            )
        })
    }

    fn chat_url(&self) -> String {
        join_url(&self.base_url, "/chat/completions")
    }

    fn embeddings_url(&self) -> String {
        join_url(&self.base_url, "/embeddings")
    }
}

impl AiProvider for OpenAiCompatibleProvider {
    fn describe_image(&self, image: &ImageInput, prompt: &str) -> Result<ImageUnderstanding> {
        let api_key = self.api_key()?;
        let image_bytes = std::fs::read(&image.path).context("failed to read image for OpenAI")?;
        let image_base64 = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        let request = build_openai_vision_request(
            &self.vision_model,
            prompt,
            &image.mime_type,
            &image_base64,
        );
        let response: Value = post_json(&self.client, &self.chat_url(), &request, Some(api_key))?;
        Ok(ImageUnderstanding {
            description: parse_openai_chat_response(&response)?,
            model: self.vision_model.clone(),
        })
    }

    fn summarize_chunks(&self, chunks: &[AiTextChunk], prompt: &str) -> Result<Summary> {
        let api_key = self.api_key()?;
        let request = build_openai_chat_request(&self.chat_model, prompt, chunks);
        let response: Value = post_json(&self.client, &self.chat_url(), &request, Some(api_key))?;
        Ok(Summary {
            text: parse_openai_chat_response(&response)?,
            model: self.chat_model.clone(),
        })
    }

    fn embed_texts(&self, texts: &[String]) -> Result<Embeddings> {
        let api_key = self.api_key()?;
        let request = build_openai_embeddings_request(&self.embedding_model, texts);
        let response: Value = post_json(
            &self.client,
            &self.embeddings_url(),
            &request,
            Some(api_key),
        )?;
        Ok(Embeddings {
            vectors: parse_openai_embeddings_response(&response)?,
            model: self.embedding_model.clone(),
        })
    }

    fn answer(&self, question: &str, contexts: &[AiTextChunk]) -> Result<Answer> {
        let api_key = self.api_key()?;
        let request = build_openai_chat_request(&self.chat_model, question, contexts);
        let response: Value = post_json(&self.client, &self.chat_url(), &request, Some(api_key))?;
        Ok(Answer {
            text: parse_openai_chat_response(&response)?,
            model: self.chat_model.clone(),
        })
    }
}

pub(crate) fn build_openai_chat_request(
    model: &str,
    question: &str,
    contexts: &[AiTextChunk],
) -> Value {
    let context_text = contexts
        .iter()
        .map(|chunk| format!("{}: {}", chunk.id, chunk.text))
        .collect::<Vec<_>>()
        .join("\n");
    json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "你是 learnBusiness 的业务文档问答助手。只根据提供的上下文回答。"
            },
            {
                "role": "user",
                "content": format!("问题：{question}\n\n上下文：\n{context_text}")
            }
        ],
        "temperature": 0.2
    })
}

pub(crate) fn build_openai_vision_request(
    model: &str,
    prompt: &str,
    mime_type: &str,
    image_base64: &str,
) -> Value {
    json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:{mime_type};base64,{image_base64}")
                        }
                    }
                ]
            }
        ],
        "temperature": 0.2
    })
}

pub(crate) fn build_openai_embeddings_request(model: &str, texts: &[String]) -> Value {
    json!({
        "model": model,
        "input": texts
    })
}

pub(crate) fn parse_openai_chat_response(response: &Value) -> Result<String> {
    response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .context("OpenAI-compatible response did not include choices[0].message.content")
}

pub(crate) fn parse_openai_embeddings_response(response: &Value) -> Result<Vec<Vec<f32>>> {
    let data = response
        .get("data")
        .and_then(Value::as_array)
        .context("OpenAI-compatible embedding response did not include data")?;
    data.iter()
        .map(|item| {
            item.get("embedding")
                .and_then(Value::as_array)
                .context("OpenAI-compatible embedding item did not include embedding")?
                .iter()
                .map(|value| {
                    value
                        .as_f64()
                        .map(|number| number as f32)
                        .context("OpenAI-compatible embedding value is not numeric")
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_answer_builds_chat_completion_request() {
        let request = build_openai_chat_request(
            "gpt-4o-mini",
            "核心流程是什么？",
            &[AiTextChunk::new("chunk-1", "申请、审核、归档")],
        );

        assert_eq!(request["model"], "gpt-4o-mini");
        assert!(
            request["messages"][1]["content"]
                .as_str()
                .unwrap()
                .contains("申请、审核、归档")
        );
    }

    #[test]
    fn openai_vision_builds_controlled_payload() {
        let request =
            build_openai_vision_request("gpt-4o-mini", "描述图片", "image/png", "aW1hZ2U=");

        assert_eq!(request["model"], "gpt-4o-mini");
        assert_eq!(
            request["messages"][0]["content"][1]["image_url"]["url"],
            "data:image/png;base64,aW1hZ2U="
        );
    }

    #[test]
    fn openai_embeddings_request_and_response_are_supported() {
        let request = build_openai_embeddings_request(
            "text-embedding-3-small",
            &["业务流程".to_string(), "审批".to_string()],
        );
        assert_eq!(request["model"], "text-embedding-3-small");
        assert_eq!(request["input"][0], "业务流程");

        let response = json!({"data": [{"embedding": [1.0, 2.0]}, {"embedding": [3.0, 4.0]}]});
        assert_eq!(
            parse_openai_embeddings_response(&response).unwrap(),
            vec![vec![1.0, 2.0], vec![3.0, 4.0]]
        );
    }
}
