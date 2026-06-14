use std::fs;
use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, params};
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
                token_estimate INTEGER,
                redaction_applied INTEGER NOT NULL DEFAULT 0,
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

    pub fn insert_chunk(
        &self,
        id: &str,
        document_id: &str,
        kind: &str,
        text: &str,
        page: Option<u32>,
        slide: Option<u32>,
    ) -> Result<()> {
        let content_hash = sha256_text(text);
        self.connection.execute(
            "
            INSERT INTO chunks (
                id,
                document_id,
                kind,
                text,
                page,
                slide,
                content_hash
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                document_id = excluded.document_id,
                kind = excluded.kind,
                text = excluded.text,
                page = excluded.page,
                slide = excluded.slide,
                content_hash = excluded.content_hash
            ",
            params![
                id,
                document_id,
                kind,
                text,
                page.map(i64::from),
                slide.map(i64::from),
                content_hash,
            ],
        )?;

        self.connection
            .execute("DELETE FROM chunks_fts WHERE chunk_id = ?1", params![id])?;
        self.connection.execute(
            "
            INSERT INTO chunks_fts (chunk_id, document_id, text)
            VALUES (?1, ?2, ?3)
            ",
            params![id, document_id, full_text_index_body(text)],
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
                bm25(chunks_fts) AS score
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
                0.0 AS score
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
}
