use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct TraceLogger {
    path: PathBuf,
    enabled: bool,
}

impl TraceLogger {
    pub fn new(path: impl Into<PathBuf>, enabled: bool) -> Self {
        Self {
            path: path.into(),
            enabled,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, event: &TraceEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(event)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceEvent {
    pub timestamp: String,
    pub trace_id: String,
    pub component: String,
    pub operation: String,
    pub status: String,
    pub provider: String,
    pub model: String,
    pub purpose: String,
    pub input_hash: String,
    pub output_hash: Option<String>,
    pub token_estimate: Option<u32>,
    pub redaction_applied: bool,
    pub local_provider: bool,
    pub error_category: Option<String>,
    pub elapsed_ms: Option<u128>,
}

impl TraceEvent {
    pub fn ai_runtime(
        trace_id: impl Into<String>,
        operation: impl Into<String>,
        status: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        purpose: impl Into<String>,
        input_hash: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now().to_rfc3339(),
            trace_id: trace_id.into(),
            component: "AiRuntime".to_string(),
            operation: operation.into(),
            status: status.into(),
            provider: provider.into(),
            model: model.into(),
            purpose: purpose.into(),
            input_hash: input_hash.into(),
            output_hash: None,
            token_estimate: None,
            redaction_applied: false,
            local_provider: false,
            error_category: None,
            elapsed_ms: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperationTraceLogger {
    path: PathBuf,
    enabled: bool,
}

impl OperationTraceLogger {
    pub fn new(path: impl Into<PathBuf>, enabled: bool) -> Self {
        Self {
            path: path.into(),
            enabled,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, event: &OperationTraceEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(event)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }

    pub fn read(&self, trace_id: Option<&str>) -> Result<Vec<OperationTraceEvent>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.path)?;
        let mut events = Vec::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let event: OperationTraceEvent = serde_json::from_str(line)?;
            if trace_id.is_none_or(|expected| event.trace_id == expected) {
                events.push(event);
            }
        }
        Ok(events)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationTraceEvent {
    pub timestamp: String,
    pub trace_id: String,
    pub operation: String,
    pub component: String,
    pub step: String,
    pub status: String,
    pub input_hash: Option<String>,
    pub output_hash: Option<String>,
    pub result_count: Option<usize>,
    pub token_estimate: Option<u32>,
    pub redaction_applied: Option<bool>,
    pub error_category: Option<String>,
    pub elapsed_ms: Option<u128>,
    pub message: Option<String>,
}

impl OperationTraceEvent {
    pub fn new(
        trace_id: impl Into<String>,
        operation: impl Into<String>,
        component: impl Into<String>,
        step: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now().to_rfc3339(),
            trace_id: trace_id.into(),
            operation: operation.into(),
            component: component.into(),
            step: step.into(),
            status: status.into(),
            input_hash: None,
            output_hash: None,
            result_count: None,
            token_estimate: None,
            redaction_applied: None,
            error_category: None,
            elapsed_ms: None,
            message: None,
        }
    }
}

pub fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn new_operation_trace_id(operation: &str, input_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(operation.as_bytes());
    hasher.update(b"\0");
    hasher.update(input_hash.as_bytes());
    hasher.update(b"\0");
    hasher.update(Utc::now().to_rfc3339().as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_logger_writes_jsonl_without_raw_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let logger = TraceLogger::new(dir.path().join("trace.jsonl"), true);
        let mut event = TraceEvent::ai_runtime(
            "trace-1",
            "provider_call",
            "failed",
            "mock",
            "mock-ai",
            "answer",
            "input-hash",
        );
        event.error_category = Some("provider_failed".to_string());
        event.token_estimate = Some(12);
        logger.append(&event).unwrap();

        let log = std::fs::read_to_string(logger.path()).unwrap();
        assert!(log.contains("\"trace_id\":\"trace-1\""));
        assert!(log.contains("\"error_category\":\"provider_failed\""));
        assert!(!log.contains("原始问题"));
        assert!(!log.contains("业务正文"));
    }

    #[test]
    fn disabled_trace_logger_does_not_create_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace.jsonl");
        let logger = TraceLogger::new(&path, false);
        let event = TraceEvent::ai_runtime(
            "trace-1",
            "provider_call",
            "started",
            "mock",
            "mock-ai",
            "answer",
            "input-hash",
        );

        logger.append(&event).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn operation_trace_logger_writes_safe_jsonl_without_raw_text() {
        let dir = tempfile::tempdir().unwrap();
        let logger = OperationTraceLogger::new(dir.path().join("operations.jsonl"), true);
        let mut event =
            OperationTraceEvent::new("trace-safe", "ask", "retrieval", "search_text", "completed");
        event.input_hash = Some("hash-only".to_string());
        event.result_count = Some(2);
        event.token_estimate = Some(120);
        event.message = Some("selected_chunks=2".to_string());
        logger.append(&event).unwrap();

        let log = std::fs::read_to_string(logger.path()).unwrap();
        assert!(log.contains("\"trace_id\":\"trace-safe\""));
        assert!(log.contains("\"result_count\":2"));
        assert!(log.contains("\"token_estimate\":120"));
        assert!(!log.contains("secret@example.com"));
        assert!(!log.contains("13800138000"));
        assert!(!log.contains("sk-live-secret"));
    }

    #[test]
    fn operation_trace_logger_can_filter_by_trace_id() {
        let dir = tempfile::tempdir().unwrap();
        let logger = OperationTraceLogger::new(dir.path().join("operations.jsonl"), true);
        logger
            .append(&OperationTraceEvent::new(
                "trace-1",
                "search",
                "store",
                "search_text",
                "completed",
            ))
            .unwrap();
        logger
            .append(&OperationTraceEvent::new(
                "trace-2",
                "ask",
                "qa",
                "answer",
                "completed",
            ))
            .unwrap();

        let events = logger.read(Some("trace-2")).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].trace_id, "trace-2");
        assert_eq!(events[0].operation, "ask");
    }

    #[test]
    fn disabled_operation_trace_logger_does_not_create_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("operations.jsonl");
        let logger = OperationTraceLogger::new(&path, false);
        let event =
            OperationTraceEvent::new("trace-1", "search", "store", "search_text", "completed");

        logger.append(&event).unwrap();
        assert!(!path.exists());
    }
}
