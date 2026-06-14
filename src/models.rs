use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub path: String,
    pub file_type: String,
    pub content_hash: String,
    pub modified_at: DateTime<Utc>,
    pub size_bytes: u64,
    pub ingest_status: IngestStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IngestStatus {
    Pending,
    Indexed,
    Skipped,
    Warning,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub document_id: String,
    pub kind: ChunkKind,
    pub text: String,
    pub page: Option<u32>,
    pub slide: Option<u32>,
    pub source_range: Option<String>,
    pub artifact_path: Option<String>,
    pub confidence: Option<u8>,
    pub ai_generated: bool,
    pub content_hash: String,
}

impl Chunk {
    pub fn stable_id(
        document_id: &str,
        kind: ChunkKind,
        page: Option<u32>,
        slide: Option<u32>,
        content_hash: &str,
    ) -> String {
        let seed = format!(
            "{document_id}|{}|{}|{}|{content_hash}",
            kind.as_str(),
            page.map_or_else(String::new, |value| value.to_string()),
            slide.map_or_else(String::new, |value| value.to_string())
        );
        Uuid::new_v5(&Uuid::NAMESPACE_URL, seed.as_bytes()).to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkKind {
    Text,
    Table,
    Image,
    Page,
    Slide,
    AiSummary,
    OcrText,
}

impl ChunkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Table => "table",
            Self::Image => "image",
            Self::Page => "page",
            Self::Slide => "slide",
            Self::AiSummary => "ai_summary",
            Self::OcrText => "ocr_text",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiCall {
    pub id: String,
    pub task_id: String,
    pub provider: String,
    pub model: String,
    pub purpose: String,
    pub input_hash: String,
    pub output_hash: Option<String>,
    pub token_estimate: Option<u32>,
    pub redaction_applied: bool,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    pub chunk_id: String,
    pub document_path: String,
    pub page: Option<u32>,
    pub slide: Option<u32>,
    pub source_range: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_id_is_stable_for_same_source() {
        let first = Chunk::stable_id("doc-1", ChunkKind::Text, Some(3), None, "hello");
        let second = Chunk::stable_id("doc-1", ChunkKind::Text, Some(3), None, "hello");
        assert_eq!(first, second);
    }
}
