use std::fs;
use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub struct MetadataStore {
    connection: Connection,
}

impl MetadataStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(path)?;
        connection.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                modified_at TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                ingest_status TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                text TEXT NOT NULL,
                page INTEGER,
                slide INTEGER,
                source_range TEXT,
                artifact_path TEXT,
                confidence INTEGER,
                ai_generated INTEGER NOT NULL DEFAULT 0,
                content_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS ai_calls (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                purpose TEXT NOT NULL,
                input_hash TEXT NOT NULL,
                output_hash TEXT,
                trace_id TEXT,
                token_estimate INTEGER,
                redaction_applied INTEGER NOT NULL DEFAULT 0,
                error_category TEXT,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                chunk_id UNINDEXED,
                document_id UNINDEXED,
                text
            );
            ",
        )?;
        ensure_ai_calls_error_category_column(&connection)?;
        ensure_ai_calls_trace_id_column(&connection)?;

        Ok(Self { connection })
    }

    pub fn upsert_document(&self, document: &DocumentRecord) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO documents (
                id,
                path,
                file_type,
                content_hash,
                modified_at,
                size_bytes,
                ingest_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                path = excluded.path,
                file_type = excluded.file_type,
                content_hash = excluded.content_hash,
                modified_at = excluded.modified_at,
                size_bytes = excluded.size_bytes,
                ingest_status = excluded.ingest_status
            ",
            params![
                document.id,
                document.path,
                document.file_type,
                document.content_hash,
                document.modified_at,
                document.size_bytes,
                document.ingest_status,
            ],
        )?;
        Ok(())
    }

    pub fn document_content_hash(&self, document_id: &str) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT content_hash FROM documents WHERE id = ?1",
                params![document_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn delete_chunks_for_document(&self, document_id: &str) -> Result<()> {
        self.connection.execute(
            "DELETE FROM chunks_fts WHERE document_id = ?1",
            params![document_id],
        )?;
        self.connection.execute(
            "DELETE FROM chunks WHERE document_id = ?1",
            params![document_id],
        )?;
        Ok(())
    }

    pub fn insert_chunk(
        &self,
        id: &str,
        document_id: &str,
        kind: &str,
        text: &str,
        page: Option<u32>,
        slide: Option<u32>,
    ) -> Result<()> {
        self.insert_chunk_with_metadata(ChunkInsert {
            id,
            document_id,
            kind,
            text,
            page,
            slide,
            source_range: None,
            artifact_path: None,
            confidence: None,
            ai_generated: false,
        })
    }

    pub fn insert_chunk_with_metadata(&self, chunk: ChunkInsert<'_>) -> Result<()> {
        let content_hash = sha256_text(chunk.text);
        self.connection.execute(
            "
            INSERT INTO chunks (
                id,
                document_id,
                kind,
                text,
                page,
                slide,
                source_range,
                artifact_path,
                confidence,
                ai_generated,
                content_hash
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(id) DO UPDATE SET
                document_id = excluded.document_id,
                kind = excluded.kind,
                text = excluded.text,
                page = excluded.page,
                slide = excluded.slide,
                source_range = excluded.source_range,
                artifact_path = excluded.artifact_path,
                confidence = excluded.confidence,
                ai_generated = excluded.ai_generated,
                content_hash = excluded.content_hash
            ",
            params![
                chunk.id,
                chunk.document_id,
                chunk.kind,
                chunk.text,
                chunk.page.map(i64::from),
                chunk.slide.map(i64::from),
                chunk.source_range,
                chunk.artifact_path,
                chunk.confidence.map(i64::from),
                i64::from(chunk.ai_generated),
                content_hash,
            ],
        )?;

        self.connection.execute(
            "DELETE FROM chunks_fts WHERE chunk_id = ?1",
            params![chunk.id],
        )?;
        self.connection.execute(
            "
            INSERT INTO chunks_fts (chunk_id, document_id, text)
            VALUES (?1, ?2, ?3)
            ",
            params![
                chunk.id,
                chunk.document_id,
                full_text_index_body(chunk.text)
            ],
        )?;

        Ok(())
    }

    pub fn search_text(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query = query.trim();
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let match_query = fts_phrase_query(query);
        let mut statement = self.connection.prepare(
            "
            SELECT
                c.id,
                d.path,
                c.text,
                bm25(chunks_fts) AS score,
                c.page,
                c.slide,
                c.source_range,
                c.artifact_path
            FROM chunks_fts
            JOIN chunks c ON c.id = chunks_fts.chunk_id
            JOIN documents d ON d.id = c.document_id
            WHERE chunks_fts MATCH ?1
            ORDER BY score
            LIMIT ?2
            ",
        )?;
        let rows = statement.query_map(params![match_query, limit as i64], |row| {
            Ok(SearchResult {
                chunk_id: row.get(0)?,
                document_path: row.get(1)?,
                snippet: row.get(2)?,
                score: row.get(3)?,
                page: row.get::<_, Option<i64>>(4)?.map(|value| value as u32),
                slide: row.get::<_, Option<i64>>(5)?.map(|value| value as u32),
                source_range: row.get(6)?,
                artifact_path: row.get(7)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn list_chunks(&self, limit: usize) -> Result<Vec<SearchResult>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let mut statement = self.connection.prepare(
            "
            SELECT
                c.id,
                d.path,
                c.text,
                0.0 AS score,
                c.page,
                c.slide,
                c.source_range,
                c.artifact_path
            FROM chunks c
            JOIN documents d ON d.id = c.document_id
            ORDER BY d.path, c.id
            LIMIT ?1
            ",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(SearchResult {
                chunk_id: row.get(0)?,
                document_path: row.get(1)?,
                snippet: row.get(2)?,
                score: row.get(3)?,
                page: row.get::<_, Option<i64>>(4)?.map(|value| value as u32),
                slide: row.get::<_, Option<i64>>(5)?.map(|value| value as u32),
                source_range: row.get(6)?,
                artifact_path: row.get(7)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn document_count(&self) -> Result<usize> {
        let count = self
            .connection
            .query_row("SELECT COUNT(*) FROM documents", [], |row| {
                row.get::<_, i64>(0)
            })?;
        Ok(count as usize)
    }

    pub fn list_documents(&self) -> Result<Vec<DocumentRecord>> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, path, file_type, content_hash, modified_at, size_bytes, ingest_status
            FROM documents
            ORDER BY path
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(DocumentRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                file_type: row.get(2)?,
                content_hash: row.get(3)?,
                modified_at: row.get(4)?,
                size_bytes: row.get::<_, i64>(5)? as u64,
                ingest_status: row.get(6)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn insert_ai_call(&self, call: &AiCallRecord) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO ai_calls (
                id,
                task_id,
                provider,
                model,
                purpose,
                input_hash,
                output_hash,
                trace_id,
                token_estimate,
                redaction_applied,
                error_category,
                status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(id) DO UPDATE SET
                output_hash = excluded.output_hash,
                trace_id = excluded.trace_id,
                token_estimate = excluded.token_estimate,
                redaction_applied = excluded.redaction_applied,
                error_category = excluded.error_category,
                status = excluded.status
            ",
            params![
                call.id,
                call.task_id,
                call.provider,
                call.model,
                call.purpose,
                call.input_hash,
                call.output_hash,
                call.trace_id,
                call.token_estimate.map(i64::from),
                i64::from(call.redaction_applied),
                call.error_category,
                call.status,
            ],
        )?;
        Ok(())
    }

    pub fn list_ai_calls(&self) -> Result<Vec<AiCallRecord>> {
        let mut statement = self.connection.prepare(
            "
            SELECT
                id,
                task_id,
                provider,
                model,
                purpose,
                input_hash,
                output_hash,
                trace_id,
                token_estimate,
                redaction_applied,
                error_category,
                status
            FROM ai_calls
            ORDER BY created_at, id
            ",
        )?;
        let rows = statement.query_map([], |row| {
            let token_estimate = row.get::<_, Option<i64>>(8)?;
            Ok(AiCallRecord {
                id: row.get(0)?,
                task_id: row.get(1)?,
                provider: row.get(2)?,
                model: row.get(3)?,
                purpose: row.get(4)?,
                input_hash: row.get(5)?,
                output_hash: row.get(6)?,
                trace_id: row.get(7)?,
                token_estimate: token_estimate.map(|value| value as u32),
                redaction_applied: row.get::<_, i64>(9)? != 0,
                error_category: row.get(10)?,
                status: row.get(11)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRecord {
    pub id: String,
    pub path: String,
    pub file_type: String,
    pub content_hash: String,
    pub modified_at: String,
    pub size_bytes: u64,
    pub ingest_status: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ChunkInsert<'a> {
    pub id: &'a str,
    pub document_id: &'a str,
    pub kind: &'a str,
    pub text: &'a str,
    pub page: Option<u32>,
    pub slide: Option<u32>,
    pub source_range: Option<&'a str>,
    pub artifact_path: Option<&'a str>,
    pub confidence: Option<u8>,
    pub ai_generated: bool,
}

impl DocumentRecord {
    pub fn new_for_test(id: &str, path: &str, file_type: &str) -> Self {
        Self {
            id: id.to_string(),
            path: path.to_string(),
            file_type: file_type.to_string(),
            content_hash: sha256_text(path),
            modified_at: Utc::now().to_rfc3339(),
            size_bytes: 0,
            ingest_status: "indexed".to_string(),
        }
    }

    pub fn new(
        id: impl Into<String>,
        path: impl Into<String>,
        file_type: impl Into<String>,
        content_hash: impl Into<String>,
        size_bytes: u64,
        ingest_status: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            file_type: file_type.into(),
            content_hash: content_hash.into(),
            modified_at: Utc::now().to_rfc3339(),
            size_bytes,
            ingest_status: ingest_status.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub chunk_id: String,
    pub document_path: String,
    pub snippet: String,
    pub score: f64,
    pub page: Option<u32>,
    pub slide: Option<u32>,
    pub source_range: Option<String>,
    pub artifact_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCallRecord {
    pub id: String,
    pub task_id: String,
    pub provider: String,
    pub model: String,
    pub purpose: String,
    pub input_hash: String,
    pub output_hash: Option<String>,
    pub trace_id: Option<String>,
    pub token_estimate: Option<u32>,
    pub redaction_applied: bool,
    pub error_category: Option<String>,
    pub status: String,
}

impl AiCallRecord {
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        purpose: impl Into<String>,
        input_hash: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        let provider = provider.into();
        let model = model.into();
        let purpose = purpose.into();
        let input_hash = input_hash.into();
        let status = status.into();
        let id = sha256_text(&format!(
            "{provider}|{model}|{purpose}|{input_hash}|{status}"
        ));
        Self {
            id,
            task_id: purpose.clone(),
            provider,
            model,
            purpose,
            input_hash,
            output_hash: None,
            trace_id: None,
            token_estimate: Some(0),
            redaction_applied: false,
            error_category: None,
            status,
        }
    }
}

fn ensure_ai_calls_trace_id_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(ai_calls)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    let has_trace_id = columns
        .collect::<rusqlite::Result<Vec<_>>>()?
        .iter()
        .any(|name| name == "trace_id");
    if !has_trace_id {
        connection.execute("ALTER TABLE ai_calls ADD COLUMN trace_id TEXT", [])?;
    }
    Ok(())
}

fn ensure_ai_calls_error_category_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(ai_calls)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    let has_error_category = columns
        .collect::<rusqlite::Result<Vec<_>>>()?
        .iter()
        .any(|name| name == "error_category");
    if !has_error_category {
        connection.execute("ALTER TABLE ai_calls ADD COLUMN error_category TEXT", [])?;
    }
    Ok(())
}

fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn full_text_index_body(text: &str) -> String {
    let mut terms = vec![text.to_string()];
    for segment in text.split_whitespace() {
        let chars = segment.chars().collect::<Vec<_>>();
        for width in [2_usize, 3] {
            if chars.len() < width {
                continue;
            }
            for window in chars.windows(width) {
                terms.push(window.iter().collect());
            }
        }
    }
    terms.join(" ")
}

fn fts_phrase_query(query: &str) -> String {
    let mut terms = query
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();

    let normalized = query
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect::<String>();
    if !normalized.is_empty() {
        terms.push(&normalized);
    }

    let chars = normalized.chars().collect::<Vec<_>>();
    let mut owned_terms = terms
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    for width in [2_usize, 3] {
        if chars.len() < width {
            continue;
        }
        for window in chars.windows(width) {
            owned_terms.push(window.iter().collect());
        }
    }

    owned_terms.sort();
    owned_terms.dedup();

    if owned_terms.is_empty() {
        "\"\"".to_string()
    } else {
        owned_terms
            .into_iter()
            .filter(|term| !term.is_empty())
            .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" OR ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_chunks_and_searches_full_text() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("metadata.sqlite");
        let store = MetadataStore::open(&db).unwrap();

        let doc = DocumentRecord::new_for_test("doc-1", "sample.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk("chunk-1", "doc-1", "text", "客户准入规则", None, None)
            .unwrap();

        let results = store.search_text("准入", 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk_id, "chunk-1");
    }

    #[test]
    fn search_results_include_chunk_location_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("metadata.sqlite")).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "slides.pptx", "application/pptx");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk_with_metadata(ChunkInsert {
                id: "chunk-slide-2",
                document_id: "doc-1",
                kind: "text",
                text: "第二页风险点",
                page: None,
                slide: Some(2),
                source_range: Some("slide:2"),
                artifact_path: Some("slides.pptx"),
                confidence: Some(95),
                ai_generated: false,
            })
            .unwrap();

        let results = store.search_text("风险点", 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk_id, "chunk-slide-2");
        assert_eq!(results[0].slide, Some(2));
        assert_eq!(results[0].source_range.as_deref(), Some("slide:2"));
        assert_eq!(results[0].artifact_path.as_deref(), Some("slides.pptx"));
    }

    #[test]
    fn stores_and_lists_ai_call_records() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("metadata.sqlite")).unwrap();
        let record = AiCallRecord::new("mock", "mock-ai", "describe_image", "abc123", "dry_run");
        store.insert_ai_call(&record).unwrap();

        let calls = store.list_ai_calls().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].purpose, "describe_image");
        assert_eq!(calls[0].status, "dry_run");
    }

    #[test]
    fn stores_ai_call_success_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("metadata.sqlite")).unwrap();
        let mut record = AiCallRecord::new(
            "mock",
            "mock-ai",
            "describe_image",
            "input-hash",
            "completed",
        );
        record.output_hash = Some("output-hash".to_string());
        record.token_estimate = Some(42);
        record.redaction_applied = true;
        store.insert_ai_call(&record).unwrap();

        let calls = store.list_ai_calls().unwrap();
        assert_eq!(calls[0].output_hash.as_deref(), Some("output-hash"));
        assert_eq!(calls[0].token_estimate, Some(42));
        assert!(calls[0].redaction_applied);
        assert_eq!(calls[0].error_category, None);
    }

    #[test]
    fn stores_ai_call_failure_category_without_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("metadata.sqlite")).unwrap();
        let mut record = AiCallRecord::new(
            "http",
            "business-chat",
            "answer",
            "input-hash-only",
            "failed",
        );
        record.error_category = Some("api_key_missing".to_string());
        store.insert_ai_call(&record).unwrap();

        let calls = store.list_ai_calls().unwrap();
        assert_eq!(calls[0].status, "failed");
        assert_eq!(calls[0].error_category.as_deref(), Some("api_key_missing"));
        assert_eq!(calls[0].input_hash, "input-hash-only");
    }

    #[test]
    fn stores_ai_call_trace_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("metadata.sqlite")).unwrap();
        let mut record = AiCallRecord::new("mock", "mock-ai", "answer", "input-hash", "completed");
        record.trace_id = Some("trace-123".to_string());
        store.insert_ai_call(&record).unwrap();

        let calls = store.list_ai_calls().unwrap();
        assert_eq!(calls[0].trace_id.as_deref(), Some("trace-123"));
    }
}
