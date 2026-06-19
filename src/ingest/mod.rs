pub mod extract;

use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::ai::AiRuntime;
use crate::config::DEFAULT_CHUNK_CHAR_LIMIT;
use crate::discover::{DiscoveredDocument, discover_documents};
use crate::ingest::extract::{ExtractedChunk, extract_document_text};
use crate::models::{Chunk, ChunkKind};
use crate::store::{ChunkInsert, DocumentRecord, MetadataStore};
use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestSummary {
    pub scanned: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub warnings: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestOptions {
    pub describe_images: bool,
    pub dry_run_ai: bool,
}

pub fn run_ingest(
    workspace_root: impl AsRef<Path>,
    docs_dir: impl AsRef<Path>,
) -> Result<IngestSummary> {
    run_ingest_with_options(workspace_root, docs_dir, IngestOptions::default())
}

pub fn run_ingest_with_options(
    workspace_root: impl AsRef<Path>,
    docs_dir: impl AsRef<Path>,
    options: IngestOptions,
) -> Result<IngestSummary> {
    let workspace = Workspace::open(workspace_root);
    let store = MetadataStore::open(workspace.metadata_db_path())?;
    let runtime = if options.describe_images {
        Some(AiRuntime::open(workspace.root())?)
    } else {
        None
    };
    let documents = discover_documents(docs_dir)?;
    let mut summary = IngestSummary {
        scanned: documents.len(),
        ..IngestSummary::default()
    };

    for document in documents {
        match ingest_one(&store, &document, runtime.as_ref(), options) {
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

fn ingest_one(
    store: &MetadataStore,
    document: &DiscoveredDocument,
    runtime: Option<&AiRuntime>,
    options: IngestOptions,
) -> Result<bool> {
    let document_id = stable_document_id(&document.path.to_string_lossy());
    if store
        .document_content_hash(&document_id)?
        .is_some_and(|existing| existing == document.sha256)
    {
        return Ok(false);
    }

    let extracted = extract_document_text(&document.path, &document.file_type)?;
    let has_text = !extracted.text.trim().is_empty()
        || extracted
            .chunks
            .iter()
            .any(|chunk| !chunk.text.trim().is_empty());
    let ingest_status = if extracted.needs_ai {
        "needs_ai"
    } else if has_text {
        "indexed"
    } else {
        "empty"
    };
    let mut record = DocumentRecord::new(
        &document_id,
        document.path.to_string_lossy(),
        &document.file_type,
        &document.sha256,
        document.size_bytes,
        ingest_status,
    );
    store.upsert_document(&record)?;
    store.delete_chunks_for_document(&document_id)?;

    if extracted.needs_ai
        && options.describe_images
        && let Some(runtime) = runtime
    {
        let result = runtime.describe_image(&document.path, options.dry_run_ai)?;
        if let Some(description) = result.description {
            let chunk_hash = sha256_text(&description);
            let chunk_id =
                Chunk::stable_id(&document_id, ChunkKind::Image, None, None, &chunk_hash);
            let artifact_path = document.path.to_string_lossy().to_string();
            store.insert_chunk_with_metadata(ChunkInsert {
                id: &chunk_id,
                document_id: &document_id,
                kind: ChunkKind::Image.as_str(),
                text: &description,
                page: None,
                slide: None,
                source_range: None,
                artifact_path: Some(&artifact_path),
                confidence: Some(80),
                ai_generated: true,
            })?;
            record.ingest_status = "indexed".to_string();
            store.upsert_document(&record)?;
            return Ok(true);
        }
    }

    if !has_text {
        return Ok(false);
    }

    let chunks = if extracted.chunks.is_empty() {
        vec![ExtractedChunk {
            text: extracted.text,
            page: None,
            slide: None,
            source_range: None,
            artifact_path: extracted.artifact_path,
            confidence: None,
            ai_generated: false,
        }]
    } else {
        extracted.chunks
    };

    let mut chunk_number = 0_u32;
    for chunk in chunks {
        for text in split_text_chunks(&chunk.text, DEFAULT_CHUNK_CHAR_LIMIT) {
            chunk_number += 1;
            let chunk_hash = sha256_text(&text);
            let chunk_id = Chunk::stable_id(
                &document_id,
                ChunkKind::Text,
                Some(chunk.page.unwrap_or(chunk_number)),
                chunk.slide,
                &chunk_hash,
            );
            let artifact_path = chunk
                .artifact_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string());
            store.insert_chunk_with_metadata(ChunkInsert {
                id: &chunk_id,
                document_id: &document_id,
                kind: "text",
                text: &text,
                page: chunk.page,
                slide: chunk.slide,
                source_range: chunk.source_range.as_deref(),
                artifact_path: artifact_path.as_deref(),
                confidence: chunk.confidence,
                ai_generated: chunk.ai_generated,
            })?;
        }
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
    use std::io::Write;
    use zip::write::SimpleFileOptions;

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

    #[test]
    fn image_documents_are_recorded_as_needing_ai_without_chunks() {
        let workspace = tempfile::tempdir().unwrap();
        let docs = tempfile::tempdir().unwrap();
        let file = docs.path().join("flow.png");
        std::fs::write(&file, b"not a real image but enough for hashing").unwrap();

        let summary = run_ingest(workspace.path(), docs.path()).unwrap();
        assert_eq!(summary.scanned, 1);
        assert_eq!(summary.warnings, 0);

        let store =
            MetadataStore::open(Workspace::open(workspace.path()).metadata_db_path()).unwrap();
        let documents = store.list_documents().unwrap();
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].ingest_status, "needs_ai");
        assert!(store.list_chunks(10).unwrap().is_empty());
    }

    #[test]
    fn image_descriptions_can_be_indexed_when_explicitly_enabled() {
        let workspace = tempfile::tempdir().unwrap();
        let docs = tempfile::tempdir().unwrap();
        let file = docs.path().join("flow.png");
        std::fs::write(&file, b"not a real image but enough for hashing").unwrap();

        let summary = run_ingest_with_options(
            workspace.path(),
            docs.path(),
            IngestOptions {
                describe_images: true,
                dry_run_ai: false,
            },
        )
        .unwrap();
        assert_eq!(summary.indexed, 1);

        let store =
            MetadataStore::open(Workspace::open(workspace.path()).metadata_db_path()).unwrap();
        let chunks = store.list_chunks(10).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].snippet.contains("mock description"));
        assert!(
            chunks[0]
                .artifact_path
                .as_deref()
                .is_some_and(|path| path.ends_with("flow.png"))
        );
        assert!(chunks[0].ai_generated);
    }

    #[test]
    fn image_description_dry_run_records_audit_without_indexing_chunk() {
        let workspace = tempfile::tempdir().unwrap();
        let docs = tempfile::tempdir().unwrap();
        let file = docs.path().join("flow.png");
        std::fs::write(&file, b"not a real image but enough for hashing").unwrap();

        let summary = run_ingest_with_options(
            workspace.path(),
            docs.path(),
            IngestOptions {
                describe_images: true,
                dry_run_ai: true,
            },
        )
        .unwrap();
        assert_eq!(summary.indexed, 0);

        let workspace_ref = Workspace::open(workspace.path());
        let store = MetadataStore::open(workspace_ref.metadata_db_path()).unwrap();
        assert!(store.list_chunks(10).unwrap().is_empty());
        let calls = store.list_ai_calls().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].status, "dry_run");
        assert_eq!(calls[0].purpose, "describe_image");
    }

    #[test]
    fn docx_ingest_indexes_extracted_text() {
        let workspace = tempfile::tempdir().unwrap();
        let docs = tempfile::tempdir().unwrap();
        let file = docs.path().join("process.docx");
        write_zip_entries(
            &file,
            &[(
                "word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>合同审批流程</w:t></w:r></w:p></w:body></w:document>"#,
            )],
        );

        let summary = run_ingest(workspace.path(), docs.path()).unwrap();
        assert_eq!(summary.indexed, 1);

        let store =
            MetadataStore::open(Workspace::open(workspace.path()).metadata_db_path()).unwrap();
        let results = store.search_text("合同审批", 5).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].document_path.ends_with("process.docx"));
    }

    #[test]
    fn pptx_ingest_indexes_slide_text_with_slide_number() {
        let workspace = tempfile::tempdir().unwrap();
        let docs = tempfile::tempdir().unwrap();
        let file = docs.path().join("deck.pptx");
        write_zip_entries(
            &file,
            &[(
                "ppt/slides/slide2.xml",
                r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>第二页风险控制</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#,
            )],
        );

        let summary = run_ingest(workspace.path(), docs.path()).unwrap();
        assert_eq!(summary.indexed, 1);

        let store =
            MetadataStore::open(Workspace::open(workspace.path()).metadata_db_path()).unwrap();
        let results = store.search_text("风险控制", 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].slide, Some(2));
    }

    fn write_zip_entries(path: &Path, entries: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, content) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
}
