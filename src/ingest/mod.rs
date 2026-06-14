pub mod extract;

use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::config::DEFAULT_CHUNK_CHAR_LIMIT;
use crate::discover::{DiscoveredDocument, discover_documents};
use crate::ingest::extract::extract_document_text;
use crate::models::{Chunk, ChunkKind};
use crate::store::{DocumentRecord, MetadataStore};
use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestSummary {
    pub scanned: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub warnings: usize,
}

pub fn run_ingest(
    workspace_root: impl AsRef<Path>,
    docs_dir: impl AsRef<Path>,
) -> Result<IngestSummary> {
    let workspace = Workspace::open(workspace_root);
    let store = MetadataStore::open(workspace.metadata_db_path())?;
    let documents = discover_documents(docs_dir)?;
    let mut summary = IngestSummary {
        scanned: documents.len(),
        ..IngestSummary::default()
    };

    for document in documents {
        match ingest_one(&store, &document) {
            Ok(indexed) => {
                if indexed {
                    summary.indexed += 1;
                } else {
                    summary.skipped += 1;
                }
            }
            Err(_) => {
                summary.warnings += 1;
            }
        }
    }

    Ok(summary)
}

fn ingest_one(store: &MetadataStore, document: &DiscoveredDocument) -> Result<bool> {
    let document_id = stable_document_id(&document.path.to_string_lossy());
    if store
        .document_content_hash(&document_id)?
        .is_some_and(|existing| existing == document.sha256)
    {
        return Ok(false);
    }

    let extracted = extract_document_text(&document.path, &document.file_type)?;
    let record = DocumentRecord::new(
        &document_id,
        document.path.to_string_lossy(),
        &document.file_type,
        &document.sha256,
        document.size_bytes,
        "indexed",
    );
    store.upsert_document(&record)?;
    store.delete_chunks_for_document(&document_id)?;

    if extracted.text.trim().is_empty() {
        return Ok(false);
    }

    for (index, text) in split_text_chunks(&extracted.text, DEFAULT_CHUNK_CHAR_LIMIT)
        .into_iter()
        .enumerate()
    {
        let chunk_hash = sha256_text(&text);
        let chunk_number = (index + 1) as u32;
        let chunk_id = Chunk::stable_id(
            &document_id,
            ChunkKind::Text,
            Some(chunk_number),
            None,
            &chunk_hash,
        );
        store.insert_chunk(
            &chunk_id,
            &document_id,
            "text",
            &text,
            Some(chunk_number),
            None,
        )?;
    }
    Ok(true)
}

fn split_text_chunks(text: &str, max_chars: usize) -> Vec<String> {
    let clean = text.trim();
    if clean.is_empty() {
        return Vec::new();
    }
    if clean.chars().count() <= max_chars {
        return vec![clean.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0_usize;
    for ch in clean.chars() {
        current.push(ch);
        current_chars += 1;
        if current_chars >= max_chars {
            chunks.push(current.trim().to_string());
            current.clear();
            current_chars = 0;
        }
    }
    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }
    chunks
}

fn stable_document_id(path: &str) -> String {
    sha256_text(path)
}

fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MetadataStore;

    #[test]
    fn repeated_ingest_skips_unchanged_and_replaces_stale_chunks() {
        let workspace = tempfile::tempdir().unwrap();
        let docs = tempfile::tempdir().unwrap();
        let file = docs.path().join("process.txt");
        std::fs::write(&file, "旧审批").unwrap();

        let first = run_ingest(workspace.path(), docs.path()).unwrap();
        assert_eq!(first.indexed, 1);

        let unchanged = run_ingest(workspace.path(), docs.path()).unwrap();
        assert_eq!(unchanged.skipped, 1);

        std::fs::write(&file, "新归档").unwrap();
        let changed = run_ingest(workspace.path(), docs.path()).unwrap();
        assert_eq!(changed.indexed, 1);

        let store =
            MetadataStore::open(Workspace::open(workspace.path()).metadata_db_path()).unwrap();
        assert!(store.search_text("旧审批", 5).unwrap().is_empty());
        assert_eq!(store.search_text("新归档", 5).unwrap().len(), 1);
    }

    #[test]
    fn long_text_is_split_into_bounded_chunks() {
        let workspace = tempfile::tempdir().unwrap();
        let docs = tempfile::tempdir().unwrap();
        let file = docs.path().join("large.txt");
        std::fs::write(&file, "业务规则".repeat(700)).unwrap();

        let summary = run_ingest(workspace.path(), docs.path()).unwrap();
        assert_eq!(summary.indexed, 1);

        let store =
            MetadataStore::open(Workspace::open(workspace.path()).metadata_db_path()).unwrap();
        let chunks = store.list_chunks(10).unwrap();
        assert!(chunks.len() > 1);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.snippet.chars().count() <= 1600)
        );
    }
}
