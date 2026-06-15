use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::ai::cache::AiCacheKey;
use crate::ai::redaction::redact_sensitive_text;
use crate::ai::{
    AiProvider, AiProviderDescriptor, AiTextChunk, ImageInput, api_key_from_env,
    provider_from_config,
};
use crate::config::AppConfig;
use crate::discover::{guess_file_type, sha256_file};
use crate::qa::QaAnswer;
use crate::store::{AiCallRecord, MetadataStore, SearchResult};
use crate::trace::{TraceEvent, TraceLogger};
use crate::workspace::Workspace;

pub struct AiRuntime {
    workspace: Workspace,
    config: AppConfig,
    descriptor: AiProviderDescriptor,
    provider: Box<dyn AiProvider>,
    trace_logger: TraceLogger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageDescriptionResult {
    pub image_path: PathBuf,
    pub description: Option<String>,
    pub provider: String,
    pub model: String,
    pub purpose: String,
    pub input_hash: String,
    pub mime_type: String,
    pub redaction_applied: bool,
    pub token_estimate: u32,
    pub local_provider: bool,
    pub status: String,
}

impl AiRuntime {
    pub fn open(workspace_root: impl AsRef<Path>) -> Result<Self> {
        let workspace = Workspace::open(workspace_root);
        let config = AppConfig::load_or_default(workspace.config_path())?;
        Self::new(workspace, config)
    }

    pub fn new(workspace: Workspace, config: AppConfig) -> Result<Self> {
        let provider = provider_from_config(&config.ai, api_key_from_env(&config.ai))?;
        Self::with_provider(workspace, config, provider)
    }

    pub fn with_provider(
        workspace: Workspace,
        config: AppConfig,
        provider: Box<dyn AiProvider>,
    ) -> Result<Self> {
        let descriptor = AiProviderDescriptor::from_config(&config.ai)?;
        let trace_logger =
            TraceLogger::new(workspace.trace_log_path(), config.logging.trace_enabled);
        Ok(Self {
            workspace,
            config,
            descriptor,
            provider,
            trace_logger,
        })
    }

    pub fn descriptor(&self) -> &AiProviderDescriptor {
        &self.descriptor
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn answer(&self, question: &str) -> Result<QaAnswer> {
        let store = MetadataStore::open(self.workspace.metadata_db_path())?;
        let results = store.search_text(question, self.config.performance.context_chunks)?;
        if results.is_empty() {
            return Ok(QaAnswer {
                answer: "未找到相关来源。".to_string(),
                sources: Vec::new(),
            });
        }

        let contexts = results
            .iter()
            .map(|result| {
                AiTextChunk::new(
                    &result.chunk_id,
                    limit_context_text(&result.snippet, self.config.performance.chunk_char_limit),
                )
            })
            .collect::<Vec<_>>();
        let redaction_applied = self.should_redact_for_provider();
        let question_for_provider = if redaction_applied {
            redact_sensitive_text(question)
        } else {
            question.to_string()
        };
        let contexts_for_provider = if redaction_applied {
            contexts
                .iter()
                .map(|chunk| AiTextChunk::new(&chunk.id, redact_sensitive_text(&chunk.text)))
                .collect::<Vec<_>>()
        } else {
            contexts
        };
        let token_estimate =
            estimate_answer_tokens(&question_for_provider, &contexts_for_provider) as u32;
        let input_hash = answer_input_hash(&question_for_provider, &contexts_for_provider);
        let trace_id = new_trace_id("answer", &input_hash);
        self.trace_ai_call(
            &trace_id,
            &AiCallAudit {
                model: &self.config.ai.chat_model,
                purpose: "answer",
                input_hash: &input_hash,
                status: "started",
                token_estimate,
                redaction_applied,
                output_hash: None,
                error_category: None,
            },
            None,
        )?;
        let started = Instant::now();
        let answer = match self
            .provider
            .answer(&question_for_provider, &contexts_for_provider)
        {
            Ok(answer) => answer,
            Err(error) => {
                let audit = AiCallAudit {
                    model: &self.config.ai.chat_model,
                    purpose: "answer",
                    input_hash: &input_hash,
                    status: "failed",
                    token_estimate,
                    redaction_applied,
                    output_hash: None,
                    error_category: Some(classify_ai_error(&error)),
                };
                self.record_ai_call(audit.clone())?;
                self.trace_ai_call(&trace_id, &audit, Some(started.elapsed().as_millis()))?;
                return Err(error).context("AI answer provider call failed");
            }
        };
        let output_hash = sha256_text(&answer.text);
        let audit = AiCallAudit {
            model: &answer.model,
            purpose: "answer",
            input_hash: &input_hash,
            status: "completed",
            token_estimate,
            redaction_applied,
            output_hash: Some(output_hash),
            error_category: None,
        };
        self.record_ai_call(audit.clone())?;
        self.trace_ai_call(&trace_id, &audit, Some(started.elapsed().as_millis()))?;
        Ok(QaAnswer {
            answer: answer.text,
            sources: unique_sources(&results),
        })
    }

    pub fn describe_image(
        &self,
        image_path: impl AsRef<Path>,
        dry_run: bool,
    ) -> Result<ImageDescriptionResult> {
        let image_path = image_path.as_ref().to_path_buf();
        let input_hash = sha256_file(&image_path)?;
        let mime_type = guess_file_type(&image_path);
        let prompt = "请描述这张业务图片中的流程、角色和关键信息。";
        let redaction_applied = self.should_redact_for_provider();
        let token_estimate = estimate_tokens(prompt) as u32;
        let trace_id = new_trace_id("describe_image", &input_hash);
        let model = if dry_run {
            self.config.ai.vision_model.clone()
        } else {
            String::new()
        };

        if dry_run {
            let audit = AiCallAudit {
                model: &model,
                purpose: "describe_image",
                input_hash: &input_hash,
                status: "dry_run",
                token_estimate,
                redaction_applied,
                output_hash: None,
                error_category: None,
            };
            self.record_ai_call(audit.clone())?;
            self.trace_ai_call(&trace_id, &audit, Some(0))?;
            return Ok(ImageDescriptionResult {
                image_path,
                description: None,
                provider: self.config.ai.provider.clone(),
                model,
                purpose: "describe_image".to_string(),
                input_hash,
                mime_type,
                redaction_applied,
                token_estimate,
                local_provider: self.descriptor.local_only,
                status: "dry_run".to_string(),
            });
        }

        let image = ImageInput::new(&image_path, &mime_type, &input_hash);
        self.trace_ai_call(
            &trace_id,
            &AiCallAudit {
                model: &self.config.ai.vision_model,
                purpose: "describe_image",
                input_hash: &input_hash,
                status: "started",
                token_estimate,
                redaction_applied,
                output_hash: None,
                error_category: None,
            },
            None,
        )?;
        let started = Instant::now();
        let understanding = match self.provider.describe_image(&image, prompt) {
            Ok(understanding) => understanding,
            Err(error) => {
                let audit = AiCallAudit {
                    model: &self.config.ai.vision_model,
                    purpose: "describe_image",
                    input_hash: &input_hash,
                    status: "failed",
                    token_estimate,
                    redaction_applied,
                    output_hash: None,
                    error_category: Some(classify_ai_error(&error)),
                };
                self.record_ai_call(audit.clone())?;
                self.trace_ai_call(&trace_id, &audit, Some(started.elapsed().as_millis()))?;
                return Err(error).context("AI image provider call failed");
            }
        };
        let output_hash = sha256_text(&understanding.description);
        let audit = AiCallAudit {
            model: &understanding.model,
            purpose: "describe_image",
            input_hash: &input_hash,
            status: "completed",
            token_estimate,
            redaction_applied,
            output_hash: Some(output_hash.clone()),
            error_category: None,
        };
        self.record_ai_call(audit.clone())?;
        self.trace_ai_call(&trace_id, &audit, Some(started.elapsed().as_millis()))?;
        self.write_ai_cache(
            &understanding.model,
            "describe_image",
            &input_hash,
            redaction_applied,
            &understanding.description,
        )?;

        Ok(ImageDescriptionResult {
            image_path,
            description: Some(understanding.description),
            provider: self.config.ai.provider.clone(),
            model: understanding.model,
            purpose: "describe_image".to_string(),
            input_hash,
            mime_type,
            redaction_applied,
            token_estimate,
            local_provider: self.descriptor.local_only,
            status: "completed".to_string(),
        })
    }

    fn should_redact_for_provider(&self) -> bool {
        self.config.safety.redact_before_external_ai && !self.descriptor.local_only
    }

    fn record_ai_call(&self, audit: AiCallAudit<'_>) -> Result<()> {
        let store = MetadataStore::open(self.workspace.metadata_db_path())?;
        let mut record = AiCallRecord::new(
            &self.config.ai.provider,
            audit.model,
            audit.purpose,
            audit.input_hash,
            audit.status,
        );
        record.token_estimate = Some(audit.token_estimate);
        record.redaction_applied = audit.redaction_applied;
        record.output_hash = audit.output_hash;
        record.error_category = audit.error_category;
        store.insert_ai_call(&record)
    }

    fn trace_ai_call(
        &self,
        trace_id: &str,
        audit: &AiCallAudit<'_>,
        elapsed_ms: Option<u128>,
    ) -> Result<()> {
        let mut event = TraceEvent::ai_runtime(
            trace_id,
            "provider_call",
            audit.status,
            &self.config.ai.provider,
            audit.model,
            audit.purpose,
            audit.input_hash,
        );
        event.output_hash = audit.output_hash.clone();
        event.token_estimate = Some(audit.token_estimate);
        event.redaction_applied = audit.redaction_applied;
        event.local_provider = self.descriptor.local_only;
        event.error_category = audit.error_category.clone();
        event.elapsed_ms = elapsed_ms;
        self.trace_logger.append(&event)
    }

    fn write_ai_cache(
        &self,
        model: &str,
        purpose: &str,
        input_hash: &str,
        redaction_applied: bool,
        content: &str,
    ) -> Result<()> {
        let cache_key = AiCacheKey::new(
            &self.config.ai.provider,
            model,
            purpose,
            "v1",
            input_hash,
            redaction_applied,
        );
        std::fs::create_dir_all(self.workspace.ai_cache_dir())?;
        std::fs::write(
            self.workspace.ai_cache_dir().join(cache_key.to_filename()),
            content,
        )?;
        Ok(())
    }
}

pub fn estimate_tokens(text: &str) -> usize {
    text.chars().filter(|ch| !ch.is_whitespace()).count()
}

#[derive(Clone)]
struct AiCallAudit<'a> {
    model: &'a str,
    purpose: &'a str,
    input_hash: &'a str,
    status: &'a str,
    token_estimate: u32,
    redaction_applied: bool,
    output_hash: Option<String>,
    error_category: Option<String>,
}

fn limit_context_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn estimate_answer_tokens(question: &str, contexts: &[AiTextChunk]) -> usize {
    estimate_tokens(question)
        + contexts
            .iter()
            .map(|chunk| estimate_tokens(&chunk.text))
            .sum::<usize>()
}

fn answer_input_hash(question: &str, contexts: &[AiTextChunk]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(question.as_bytes());
    for context in contexts {
        hasher.update(b"\0");
        hasher.update(context.id.as_bytes());
        hasher.update(b"\0");
        hasher.update(context.text.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn unique_sources(results: &[SearchResult]) -> Vec<String> {
    let mut sources = results
        .iter()
        .map(|result| result.document_path.clone())
        .collect::<Vec<_>>();
    sources.sort();
    sources.dedup();
    sources
}

fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn new_trace_id(purpose: &str, input_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(purpose.as_bytes());
    hasher.update(b"\0");
    hasher.update(input_hash.as_bytes());
    hasher.update(b"\0");
    hasher.update(Utc::now().to_rfc3339().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn classify_ai_error(error: &anyhow::Error) -> String {
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("api key") {
        "api_key_missing".to_string()
    } else if text.contains("localhost") || text.contains("unsupported ai provider") {
        "configuration".to_string()
    } else if text.contains("http") || text.contains("status") {
        "http_request".to_string()
    } else if text.contains("json") || text.contains("parse") {
        "invalid_response".to_string()
    } else {
        "provider_failed".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, bail};

    use crate::ai::{Answer, Embeddings, ImageUnderstanding, Summary};
    use crate::config::{AiConfig, PerformanceConfig, SafetyConfig};
    use crate::store::DocumentRecord;

    #[test]
    fn estimate_tokens_handles_chinese_english_empty_and_long_text() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hello world"), 10);
        assert_eq!(estimate_tokens("业务流程"), 4);
        assert_eq!(estimate_tokens(&"a".repeat(2048)), 2048);
    }

    #[test]
    fn runtime_answer_truncates_context_to_chunk_char_limit() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::init(dir.path()).unwrap();
        let store = MetadataStore::open(workspace.metadata_db_path()).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "long.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk(
                "chunk-1",
                "doc-1",
                "text",
                "共同流程包含ABCDEFGHIJK。",
                None,
                None,
            )
            .unwrap();

        let config = AppConfig {
            ai: AiConfig::default(),
            safety: SafetyConfig::default(),
            performance: PerformanceConfig {
                context_chunks: 1,
                chunk_char_limit: 6,
            },
            logging: Default::default(),
        };
        let runtime =
            AiRuntime::with_provider(workspace, config, Box::new(EchoContextProvider)).unwrap();
        let answer = runtime.answer("共同流程").unwrap();

        assert!(answer.answer.contains("共同流程包含"));
        assert!(!answer.answer.contains("A"));
    }

    #[test]
    fn runtime_provider_failure_writes_failure_audit() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::init(dir.path()).unwrap();
        let store = MetadataStore::open(workspace.metadata_db_path()).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "failure.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk("chunk-1", "doc-1", "text", "失败审计测试内容", None, None)
            .unwrap();

        let config = AppConfig::default();
        let runtime =
            AiRuntime::with_provider(workspace, config, Box::new(FailingProvider)).unwrap();
        let error = runtime.answer("为什么失败？").unwrap_err().to_string();
        assert!(error.contains("AI answer provider call failed"));

        let calls = store.list_ai_calls().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].status, "failed");
        assert_eq!(calls[0].error_category.as_deref(), Some("provider_failed"));
        assert_eq!(calls[0].purpose, "answer");
    }

    #[test]
    fn runtime_provider_failure_writes_safe_trace_log() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::init(dir.path()).unwrap();
        let store = MetadataStore::open(workspace.metadata_db_path()).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "trace.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk(
                "chunk-1",
                "doc-1",
                "text",
                "敏感业务正文 13800138000",
                None,
                None,
            )
            .unwrap();

        let trace_path = workspace.trace_log_path();
        let runtime =
            AiRuntime::with_provider(workspace, AppConfig::default(), Box::new(FailingProvider))
                .unwrap();
        let _ = runtime.answer("为什么失败 13800138000？").unwrap_err();

        let trace_log = std::fs::read_to_string(trace_path).unwrap();
        assert!(trace_log.contains("\"status\":\"failed\""));
        assert!(trace_log.contains("\"error_category\":\"provider_failed\""));
        assert!(trace_log.contains("\"purpose\":\"answer\""));
        assert!(!trace_log.contains("敏感业务正文"));
        assert!(!trace_log.contains("为什么失败"));
        assert!(!trace_log.contains("13800138000"));
    }

    #[test]
    fn runtime_redacts_external_provider_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::init(dir.path()).unwrap();
        let store = MetadataStore::open(workspace.metadata_db_path()).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "secret.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk(
                "chunk-1",
                "doc-1",
                "text",
                "联系人 test@example.com 电话 13800138000",
                None,
                None,
            )
            .unwrap();

        let config = AppConfig {
            ai: AiConfig {
                provider: "openai-compatible".to_string(),
                base_url: "https://gateway.example.com/v1".to_string(),
                chat_model: "chat".to_string(),
                vision_model: "vision".to_string(),
                embedding_model: "embedding".to_string(),
                api_key_env: "LEARNBUSINESS_TEST_KEY".to_string(),
            },
            safety: SafetyConfig::default(),
            performance: PerformanceConfig::default(),
            logging: Default::default(),
        };
        let runtime =
            AiRuntime::with_provider(workspace, config, Box::new(EchoContextProvider)).unwrap();
        let answer = runtime.answer("联系人").unwrap();

        assert!(answer.answer.contains("[REDACTED_EMAIL]"));
        assert!(answer.answer.contains("[REDACTED_PHONE]"));
        assert!(!answer.answer.contains("test@example.com"));
        let calls = store.list_ai_calls().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].redaction_applied);
    }

    struct EchoContextProvider;

    impl AiProvider for EchoContextProvider {
        fn describe_image(&self, _image: &ImageInput, _prompt: &str) -> Result<ImageUnderstanding> {
            unreachable!()
        }

        fn summarize_chunks(&self, _chunks: &[AiTextChunk], _prompt: &str) -> Result<Summary> {
            unreachable!()
        }

        fn embed_texts(&self, _texts: &[String]) -> Result<Embeddings> {
            unreachable!()
        }

        fn answer(&self, question: &str, contexts: &[AiTextChunk]) -> Result<Answer> {
            Ok(Answer {
                text: format!(
                    "{} {}",
                    question,
                    contexts
                        .iter()
                        .map(|chunk| chunk.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
                model: "echo".to_string(),
            })
        }
    }

    struct FailingProvider;

    impl AiProvider for FailingProvider {
        fn describe_image(&self, _image: &ImageInput, _prompt: &str) -> Result<ImageUnderstanding> {
            unreachable!()
        }

        fn summarize_chunks(&self, _chunks: &[AiTextChunk], _prompt: &str) -> Result<Summary> {
            unreachable!()
        }

        fn embed_texts(&self, _texts: &[String]) -> Result<Embeddings> {
            unreachable!()
        }

        fn answer(&self, _question: &str, _contexts: &[AiTextChunk]) -> Result<Answer> {
            bail!("synthetic provider failure")
        }
    }
}
