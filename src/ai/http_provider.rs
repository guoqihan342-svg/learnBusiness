use anyhow::{Context, Result};
use base64::Engine;
use serde_json::{Value, json};

use crate::ai::http::{join_url, post_json};
use crate::ai::{
    AiProvider, AiTextChunk, Answer, Embeddings, ImageInput, ImageUnderstanding, Summary,
    headers_from_config,
};
use crate::config::AiConfig;

#[derive(Debug, Clone)]
pub struct HttpAiProvider {
    pub config: AiConfig,
    client: reqwest::blocking::Client,
}

impl HttpAiProvider {
    pub fn from_config(config: &AiConfig) -> Self {
        Self {
            config: config.clone(),
            client: reqwest::blocking::Client::builder()
                .no_proxy()
                .build()
                .expect("AI HTTP client configuration should be valid"),
        }
    }

    fn chat_url(&self) -> String {
        join_url(&self.config.base_url, "/chat/completions")
    }

    fn embeddings_url(&self) -> String {
        join_url(&self.config.base_url, "/embeddings")
    }
}

impl AiProvider for HttpAiProvider {
    fn describe_image(&self, image: &ImageInput, prompt: &str) -> Result<ImageUnderstanding> {
        let image_bytes =
            std::fs::read(&image.path).context("failed to read image for HTTP AI provider")?;
        let image_base64 = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        let request = build_openai_vision_request(
            &self.config.vision_model,
            prompt,
            &image.mime_type,
            &image_base64,
        );
        let headers = headers_from_config(&self.config)?;
        let response: Value = post_json(&self.client, &self.chat_url(), &request, &headers)?;
        Ok(ImageUnderstanding {
            description: parse_openai_chat_response(&response)?,
            model: self.config.vision_model.clone(),
        })
    }

    fn summarize_chunks(&self, chunks: &[AiTextChunk], prompt: &str) -> Result<Summary> {
        let request = build_openai_chat_request(&self.config.chat_model, prompt, chunks);
        let headers = headers_from_config(&self.config)?;
        let response: Value = post_json(&self.client, &self.chat_url(), &request, &headers)?;
        Ok(Summary {
            text: parse_openai_chat_response(&response)?,
            model: self.config.chat_model.clone(),
        })
    }

    fn embed_texts(&self, texts: &[String]) -> Result<Embeddings> {
        let request = build_openai_embeddings_request(&self.config.embedding_model, texts);
        let headers = headers_from_config(&self.config)?;
        let response: Value = post_json(&self.client, &self.embeddings_url(), &request, &headers)?;
        Ok(Embeddings {
            vectors: parse_openai_embeddings_response(&response)?,
            model: self.config.embedding_model.clone(),
        })
    }

    fn answer(&self, question: &str, contexts: &[AiTextChunk]) -> Result<Answer> {
        let request = build_openai_chat_request(&self.config.chat_model, question, contexts);
        let headers = headers_from_config(&self.config)?;
        let response: Value = post_json(&self.client, &self.chat_url(), &request, &headers)?;
        Ok(Answer {
            text: parse_openai_chat_response(&response)?,
            model: self.config.chat_model.clone(),
        })
    }
}

pub type OpenAiCompatibleProvider = HttpAiProvider;

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
        .context("HTTP AI response did not include choices[0].message.content")
}

pub(crate) fn parse_openai_embeddings_response(response: &Value) -> Result<Vec<Vec<f32>>> {
    let data = response
        .get("data")
        .and_then(Value::as_array)
        .context("HTTP AI embedding response did not include data")?;
    data.iter()
        .map(|item| {
            item.get("embedding")
                .and_then(Value::as_array)
                .context("HTTP AI embedding item did not include embedding")?
                .iter()
                .map(|value| {
                    value
                        .as_f64()
                        .map(|number| number as f32)
                        .context("HTTP AI embedding value is not numeric")
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn http_answer_builds_chat_completion_request() {
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
    fn http_provider_describe_image_uses_configured_base_url_and_headers() {
        let (base_url, request_rx) =
            one_shot_json_server(r#"{"choices":[{"message":{"content":"image description"}}]}"#);
        let mut headers = BTreeMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer ${LEARNBUSINESS_VISION_HEADER_TEST_KEY}".to_string(),
        );
        headers.insert("X-App".to_string(), "learnBusiness".to_string());
        unsafe {
            std::env::set_var("LEARNBUSINESS_VISION_HEADER_TEST_KEY", "vision-secret");
        }
        let provider = HttpAiProvider::from_config(&AiConfig {
            provider: "http".to_string(),
            base_url,
            chat_model: "chat".to_string(),
            vision_model: "vision".to_string(),
            embedding_model: "embedding".to_string(),
            api_key_env: String::new(),
            headers,
        });
        let dir = tempfile::tempdir().unwrap();
        let image_path = dir.path().join("diagram.png");
        std::fs::write(&image_path, b"image-bytes").unwrap();
        let image = ImageInput::new(&image_path, "image/png", "hash");

        let understanding = provider.describe_image(&image, "describe").unwrap();
        assert_eq!(understanding.description, "image description");
        let request = request_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let request_lower = request.to_ascii_lowercase();
        assert!(request.starts_with("POST /v1/chat/completions "));
        assert!(request_lower.contains("authorization: bearer vision-secret"));
        assert!(request_lower.contains("x-app: learnbusiness"));
        assert!(request.contains("data:image/png;base64,"));
        unsafe {
            std::env::remove_var("LEARNBUSINESS_VISION_HEADER_TEST_KEY");
        }
    }

    #[test]
    fn http_vision_builds_controlled_payload() {
        let request =
            build_openai_vision_request("gpt-4o-mini", "描述图片", "image/png", "aW1hZ2U=");

        assert_eq!(request["model"], "gpt-4o-mini");
        assert_eq!(
            request["messages"][0]["content"][1]["image_url"]["url"],
            "data:image/png;base64,aW1hZ2U="
        );
    }

    #[test]
    fn http_embeddings_request_and_response_are_supported() {
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

    fn one_shot_json_server(response_body: &'static str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        buffer.extend_from_slice(&chunk[..n]);
                        if request_is_complete(&buffer) {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let request = String::from_utf8_lossy(&buffer).to_string();
            tx.send(request).unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        (format!("http://{address}/v1"), rx)
    }

    fn request_is_complete(buffer: &[u8]) -> bool {
        let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
            return false;
        };
        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let content_length = headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        });
        match content_length {
            Some(length) => buffer.len() >= header_end + 4 + length,
            None => true,
        }
    }
}
