use anyhow::{Context, Result};
use base64::Engine;
use serde_json::{Value, json};

use crate::ai::http::{join_url, post_json};
use crate::ai::{
    AiProvider, AiTextChunk, Answer, Embeddings, ImageInput, ImageUnderstanding, Summary,
};
use crate::config::AiConfig;

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    pub base_url: String,
    pub chat_model: String,
    pub vision_model: String,
    pub embedding_model: String,
    client: reqwest::blocking::Client,
}

impl OllamaProvider {
    pub fn from_config(config: &AiConfig) -> Self {
        Self {
            base_url: config.base_url.clone(),
            chat_model: config.chat_model.clone(),
            vision_model: config.vision_model.clone(),
            embedding_model: config.embedding_model.clone(),
            client: reqwest::blocking::Client::new(),
        }
    }

    fn chat_url(&self) -> String {
        join_url(&self.base_url, "/api/chat")
    }

    fn embeddings_url(&self) -> String {
        join_url(&self.base_url, "/api/embeddings")
    }
}

impl AiProvider for OllamaProvider {
    fn describe_image(&self, image: &ImageInput, prompt: &str) -> Result<ImageUnderstanding> {
        let image_bytes = std::fs::read(&image.path).context("failed to read image for Ollama")?;
        let image_base64 = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        let request = build_ollama_vision_request(&self.vision_model, prompt, &image_base64);
        let response: Value = post_json(&self.client, &self.chat_url(), &request, None)?;
        Ok(ImageUnderstanding {
            description: parse_ollama_chat_response(&response)?,
            model: self.vision_model.clone(),
        })
    }

    fn summarize_chunks(&self, chunks: &[AiTextChunk], prompt: &str) -> Result<Summary> {
        let request = build_ollama_chat_request(&self.chat_model, prompt, chunks);
        let response: Value = post_json(&self.client, &self.chat_url(), &request, None)?;
        Ok(Summary {
            text: parse_ollama_chat_response(&response)?,
            model: self.chat_model.clone(),
        })
    }

    fn embed_texts(&self, texts: &[String]) -> Result<Embeddings> {
        let mut vectors = Vec::with_capacity(texts.len());
        for text in texts {
            let request = build_ollama_embedding_request(&self.embedding_model, text);
            let response: Value = post_json(&self.client, &self.embeddings_url(), &request, None)?;
            vectors.push(parse_ollama_embedding_response(&response)?);
        }
        Ok(Embeddings {
            vectors,
            model: self.embedding_model.clone(),
        })
    }

    fn answer(&self, question: &str, contexts: &[AiTextChunk]) -> Result<Answer> {
        let request = build_ollama_chat_request(&self.chat_model, question, contexts);
        let response: Value = post_json(&self.client, &self.chat_url(), &request, None)?;
        Ok(Answer {
            text: parse_ollama_chat_response(&response)?,
            model: self.chat_model.clone(),
        })
    }
}

pub(crate) fn build_ollama_chat_request(
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
        "stream": false,
        "messages": [
            {
                "role": "system",
                "content": "你是 learnBusiness 的业务文档问答助手。只根据提供的上下文回答。"
            },
            {
                "role": "user",
                "content": format!("问题：{question}\n\n上下文：\n{context_text}")
            }
        ]
    })
}

pub(crate) fn build_ollama_vision_request(model: &str, prompt: &str, image_base64: &str) -> Value {
    json!({
        "model": model,
        "stream": false,
        "messages": [
            {
                "role": "user",
                "content": prompt,
                "images": [image_base64]
            }
        ]
    })
}

pub(crate) fn build_ollama_embedding_request(model: &str, text: &str) -> Value {
    json!({
        "model": model,
        "prompt": text
    })
}

pub(crate) fn parse_ollama_chat_response(response: &Value) -> Result<String> {
    response
        .pointer("/message/content")
        .or_else(|| response.get("response"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .context("Ollama response did not include message content")
}

pub(crate) fn parse_ollama_embedding_response(response: &Value) -> Result<Vec<f32>> {
    response
        .get("embedding")
        .and_then(Value::as_array)
        .context("Ollama embedding response did not include embedding")?
        .iter()
        .map(|value| {
            value
                .as_f64()
                .map(|number| number as f32)
                .context("Ollama embedding value is not numeric")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_answer_builds_chat_request() {
        let request = build_ollama_chat_request(
            "qwen2.5",
            "核心流程是什么？",
            &[AiTextChunk::new("chunk-1", "申请、审核、归档")],
        );

        assert_eq!(request["model"], "qwen2.5");
        assert_eq!(request["stream"], false);
        assert!(
            request["messages"][1]["content"]
                .as_str()
                .unwrap()
                .contains("申请、审核、归档")
        );
    }

    #[test]
    fn ollama_describe_image_puts_base64_only_in_payload() {
        let request = build_ollama_vision_request("llava", "描述图片", "aW1hZ2U=");

        assert_eq!(request["model"], "llava");
        assert_eq!(request["stream"], false);
        assert_eq!(request["messages"][0]["images"][0], "aW1hZ2U=");
    }

    #[test]
    fn ollama_embed_texts_builds_embedding_request_and_parses_response() {
        let request = build_ollama_embedding_request("nomic-embed-text", "业务流程");
        assert_eq!(request["model"], "nomic-embed-text");
        assert_eq!(request["prompt"], "业务流程");

        let response = json!({"embedding": [1.0, 2.5]});
        assert_eq!(
            parse_ollama_embedding_response(&response).unwrap(),
            vec![1.0, 2.5]
        );
    }
}
