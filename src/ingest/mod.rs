pub mod extract;

use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};

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

    if extracted.text.trim().is_empty() {
        return Ok(false);
    }

    let chunk_hash = sha256_text(&extracted.text);
    let chunk_id = Chunk::stable_id(&document_id, ChunkKind::Text, None, None, &chunk_hash);
    store.insert_chunk(&chunk_id, &document_id, "text", &extracted.text, None, None)?;
    Ok(true)
}

fn stable_document_id(path: &str) -> String {
    sha256_text(path)
}

fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}
