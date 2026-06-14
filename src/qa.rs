use anyhow::Result;

use crate::ai::{AiProvider, AiTextChunk, MockAiProvider};
use crate::store::{MetadataStore, SearchResult};
use crate::workspace::Workspace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QaAnswer {
    pub answer: String,
    pub sources: Vec<String>,
}

pub struct QaEngine<P: AiProvider> {
    provider: P,
}

impl Default for QaEngine<MockAiProvider> {
    fn default() -> Self {
        Self {
            provider: MockAiProvider::default(),
        }
    }
}

impl<P: AiProvider> QaEngine<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn answer(&self, store: &MetadataStore, question: &str) -> Result<QaAnswer> {
        let mut results = store.search_text(question, 5)?;
        if results.is_empty() {
            results = store.list_chunks(5)?;
        }

        let contexts = results
            .iter()
            .map(|result| AiTextChunk::new(&result.chunk_id, &result.snippet))
            .collect::<Vec<_>>();
        let answer = self.provider.answer(question, &contexts)?;
        Ok(QaAnswer {
            answer: answer.text,
            sources: unique_sources(&results),
        })
    }
}

pub fn answer_workspace(workspace_root: impl AsRef<std::path::Path>, question: &str) -> Result<QaAnswer> {
    let workspace = Workspace::open(workspace_root);
    let store = MetadataStore::open(workspace.metadata_db_path())?;
    QaEngine::default().answer(&store, question)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::DocumentRecord;

    #[test]
    fn answers_with_sources_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("metadata.sqlite")).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "process.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk("chunk-1", "doc-1", "text", "核心流程是申请、审核、归档。", None, None)
            .unwrap();

        let answer = QaEngine::default().answer(&store, "核心流程是什么？").unwrap();
        assert!(answer.sources.iter().any(|source| source.ends_with("process.txt")));
    }
}
