use anyhow::{Context, Result};
use base64::Engine;
use serde_json::{Value, json};

use crate::ai::http::{join_url, post_json};
use crate::ai::{
    AiProvider, AiTextChunk, Answer, Embeddings, ImageInput, ImageUnderstanding, Summary,
};
use crate::config::AiConfig;

#[derive(Debug, Clone)]
pub struct LocalHttpProvider {
    pub base_url: String,
    pub chat_model: String,
    pub vision_model: String,
    pub embedding_model: String,
    client: reqwest::blocking::Client,
}

impl LocalHttpProvider {
    pub fn from_config(config: &AiConfig) -> Self {
        Self {
            base_url: config.base_url.clone(),
            chat_model: config.chat_model.clone(),
            vision_model: config.vision_model.clone(),
            embedding_model: config.embedding_model.clone(),
            client: reqwest::blocking::Client::new(),
        }
    }

    fn answer_url(&self) -> String {
        join_url(&self.base_url, "/answer")
    }

    fn image_url(&self) -> String {
        join_url(&self.base_url, "/describe-image")
    }

    fn embeddings_url(&self) -> String {
        join_url(&self.base_url, "/embeddings")
    }
}

impl AiProvider for LocalHttpProvider {
    fn describe_image(&self, image: &ImageInput, prompt: &str) -> Result<ImageUnderstanding> {
        let image_bytes =
            std::fs::read(&image.path).context("failed to read image for local-http")?;
        let image_base64 = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        let request = build_local_http_image_request(
            &self.vision_model,
            prompt,
            &image.mime_type,
            &image.content_hash,
            &image_base64,
        );
        let response: Value = post_json(&self.client, &self.image_url(), &request, None)?;
        Ok(ImageUnderstanding {
            description: parse_local_http_text_response(&response, "description")?,
            model: response
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&self.vision_model)
                .to_string(),
        })
    }

    fn summarize_chunks(&self, chunks: &[AiTextChunk], prompt: &str) -> Result<Summary> {
        let request = build_local_http_answer_request(&self.chat_model, prompt, chunks);
        let response: Value = post_json(&self.client, &self.answer_url(), &request, None)?;
        Ok(Summary {
            text: parse_local_http_text_response(&response, "answer")?,
            model: response
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&self.chat_model)
                .to_string(),
        })
    }

    fn embed_texts(&self, texts: &[String]) -> Result<Embeddings> {
        let request = build_local_http_embeddings_request(&self.embedding_model, texts);
        let response: Value = post_json(&self.client, &self.embeddings_url(), &request, None)?;
        Ok(Embeddings {
            vectors: parse_local_http_embeddings_response(&response)?,
            model: response
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&self.embedding_model)
                .to_string(),
        })
    }

    fn answer(&self, question: &str, contexts: &[AiTextChunk]) -> Result<Answer> {
        let request = build_local_http_answer_request(&self.chat_model, question, contexts);
        let response: Value = post_json(&self.client, &self.answer_url(), &request, None)?;
        Ok(Answer {
            text: parse_local_http_text_response(&response, "answer")?,
            model: response
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&self.chat_model)
                .to_string(),
        })
    }
}

pub(crate) fn build_local_http_answer_request(
    model: &str,
    question: &str,
    contexts: &[AiTextChunk],
) -> Value {
    let contexts = contexts
        .iter()
        .map(|chunk| json!({"id": chunk.id, "text": chunk.text}))
        .collect::<Vec<_>>();
    json!({
        "purpose": "answer",
        "model": model,
        "question": question,
        "contexts": contexts
    })
}

pub(crate) fn build_local_http_image_request(
    model: &str,
    prompt: &str,
    mime_type: &str,
    content_hash: &str,
    image_base64: &str,
) -> Value {
    json!({
        "purpose": "describe_image",
        "model": model,
        "prompt": prompt,
        "image": {
            "mime_type": mime_type,
            "content_hash": content_hash,
            "base64": image_base64
        }
    })
}

pub(crate) fn build_local_http_embeddings_request(model: &str, texts: &[String]) -> Value {
    json!({
        "purpose": "embed_texts",
        "model": model,
        "texts": texts
    })
}

pub(crate) fn parse_local_http_text_response(response: &Value, field: &str) -> Result<String> {
    response
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .with_context(|| format!("local-http response did not include {field}"))
}

pub(crate) fn parse_local_http_embeddings_response(response: &Value) -> Result<Vec<Vec<f32>>> {
    response
        .get("embeddings")
        .and_then(Value::as_array)
        .context("local-http response did not include embeddings")?
        .iter()
        .map(|vector| {
            vector
                .as_array()
                .context("local-http embedding item is not an array")?
                .iter()
                .map(|value| {
                    value
                        .as_f64()
                        .map(|number| number as f32)
                        .context("local-http embedding value is not numeric")
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_http_answer_uses_minimal_protocol() {
        let request = build_local_http_answer_request(
            "local-chat",
            "核心流程是什么？",
            &[AiTextChunk::new("chunk-1", "申请、审核、归档")],
        );

        assert_eq!(request["purpose"], "answer");
        assert_eq!(request["model"], "local-chat");
        assert_eq!(request["contexts"][0]["id"], "chunk-1");
        assert_eq!(request["contexts"][0]["text"], "申请、审核、归档");
    }

    #[test]
    fn local_http_describe_image_uses_minimal_protocol() {
        let request = build_local_http_image_request(
            "local-vision",
            "描述图片",
            "image/png",
            "hash",
            "aW1hZ2U=",
        );

        assert_eq!(request["purpose"], "describe_image");
        assert_eq!(request["image"]["mime_type"], "image/png");
        assert_eq!(request["image"]["content_hash"], "hash");
        assert_eq!(request["image"]["base64"], "aW1hZ2U=");
    }

    #[test]
    fn local_http_embed_texts_uses_minimal_protocol() {
        let request = build_local_http_embeddings_request(
            "local-embedding",
            &["业务流程".to_string(), "审批".to_string()],
        );
        assert_eq!(request["purpose"], "embed_texts");
        assert_eq!(request["texts"][1], "审批");

        let response = json!({"embeddings": [[1.0, 2.0], [3.0, 4.0]], "model": "local-embedding"});
        assert_eq!(
            parse_local_http_embeddings_response(&response).unwrap(),
            vec![vec![1.0, 2.0], vec![3.0, 4.0]]
        );
    }
}
