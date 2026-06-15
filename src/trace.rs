use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

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
}
