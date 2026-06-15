use anyhow::Result;

use crate::ai::{AiProvider, AiTextChunk, MockAiProvider};
use crate::config::{AppConfig, DEFAULT_CONTEXT_CHUNKS};
use crate::store::{MetadataStore, SearchResult};
use crate::workspace::Workspace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QaAnswer {
    pub answer: String,
    pub sources: Vec<String>,
}

pub struct QaEngine<P: AiProvider> {
    provider: P,
    context_chunks: usize,
}

impl Default for QaEngine<MockAiProvider> {
    fn default() -> Self {
        Self {
            provider: MockAiProvider::default(),
            context_chunks: DEFAULT_CONTEXT_CHUNKS,
        }
    }
}

impl<P: AiProvider> QaEngine<P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            context_chunks: DEFAULT_CONTEXT_CHUNKS,
        }
    }

    pub fn with_context_chunks(mut self, context_chunks: usize) -> Self {
        self.context_chunks = context_chunks;
        self
    }

    pub fn answer(&self, store: &MetadataStore, question: &str) -> Result<QaAnswer> {
        let results = store.search_text(question, self.context_chunks)?;
        if results.is_empty() {
            return Ok(QaAnswer {
                answer: "未找到相关来源。".to_string(),
                sources: Vec::new(),
            });
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

pub fn answer_workspace(
    workspace_root: impl AsRef<std::path::Path>,
    question: &str,
) -> Result<QaAnswer> {
    let workspace = Workspace::open(workspace_root);
    let config = AppConfig::load_or_default(workspace.config_path())?;
    let store = MetadataStore::open(workspace.metadata_db_path())?;
    QaEngine::default()
        .with_context_chunks(config.performance.context_chunks)
        .answer(&store, question)
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
    use crate::ai::{Answer, Embeddings, ImageInput, ImageUnderstanding, Summary};
    use crate::store::DocumentRecord;

    #[test]
    fn answers_with_sources_from_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("metadata.sqlite")).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "process.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk(
                "chunk-1",
                "doc-1",
                "text",
                "核心流程是申请、审核、归档。",
                None,
                None,
            )
            .unwrap();

        let answer = QaEngine::default()
            .answer(&store, "核心流程是什么？")
            .unwrap();
        assert!(
            answer
                .sources
                .iter()
                .any(|source| source.ends_with("process.txt"))
        );
    }

    #[test]
    fn no_match_returns_no_sources_without_calling_ai() {
        struct NoCallProvider;

        impl AiProvider for NoCallProvider {
            fn describe_image(
                &self,
                _image: &ImageInput,
                _prompt: &str,
            ) -> Result<ImageUnderstanding> {
                panic!("AI should not be called for no-match questions")
            }

            fn summarize_chunks(&self, _chunks: &[AiTextChunk], _prompt: &str) -> Result<Summary> {
                panic!("AI should not be called for no-match questions")
            }

            fn embed_texts(&self, _texts: &[String]) -> Result<Embeddings> {
                panic!("AI should not be called for no-match questions")
            }

            fn answer(&self, _question: &str, _contexts: &[AiTextChunk]) -> Result<Answer> {
                panic!("AI should not be called for no-match questions")
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("metadata.sqlite")).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "process.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk("chunk-1", "doc-1", "text", "客户准入规则", None, None)
            .unwrap();

        let answer = QaEngine::new(NoCallProvider)
            .answer(&store, "完全无关的问题")
            .unwrap();
        assert!(answer.sources.is_empty());
        assert!(answer.answer.contains("未找到相关来源"));
    }

    #[test]
    fn answer_workspace_uses_configured_context_chunk_limit() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::init(dir.path()).unwrap();
        std::fs::write(
            workspace.config_path(),
            "\
[performance]
context_chunks = 1
chunk_char_limit = 1600
",
        )
        .unwrap();

        let store = MetadataStore::open(workspace.metadata_db_path()).unwrap();
        for index in 1..=2 {
            let doc_id = format!("doc-{index}");
            let path = format!("process-{index}.txt");
            let doc = DocumentRecord::new_for_test(&doc_id, &path, "text/plain");
            store.upsert_document(&doc).unwrap();
            store
                .insert_chunk(
                    &format!("chunk-{index}"),
                    &doc_id,
                    "text",
                    "共同流程包含客户申请和运营审核。",
                    None,
                    None,
                )
                .unwrap();
        }

        let answer = answer_workspace(dir.path(), "共同流程是什么？").unwrap();
        assert_eq!(answer.sources.len(), 1);
    }
}
