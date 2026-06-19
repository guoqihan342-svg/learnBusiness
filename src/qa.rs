use anyhow::Result;

use crate::ai::{AiProvider, AiRuntime, AiTextChunk, MockAiProvider};
use crate::config::DEFAULT_CONTEXT_CHUNKS;
use crate::models::Citation;
use crate::store::{MetadataStore, SearchResult};

#[derive(Debug, Clone, PartialEq)]
pub struct QaAnswer {
    pub answer: String,
    pub citations: Vec<Citation>,
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
                citations: Vec::new(),
            });
        }

        let contexts = results
            .iter()
            .map(|result| AiTextChunk::new(&result.chunk_id, &result.snippet))
            .collect::<Vec<_>>();
        let answer = self.provider.answer(question, &contexts)?;
        Ok(QaAnswer {
            answer: answer.text,
            citations: citations_from_results(&results),
        })
    }
}

pub fn answer_workspace(
    workspace_root: impl AsRef<std::path::Path>,
    question: &str,
) -> Result<QaAnswer> {
    AiRuntime::open(workspace_root)?.answer(question)
}

pub fn citations_from_results(results: &[SearchResult]) -> Vec<Citation> {
    let mut citations = results
        .iter()
        .map(|result| Citation {
            chunk_id: result.chunk_id.clone(),
            document_path: result.document_path.clone(),
            page: result.page,
            slide: result.slide,
            source_range: result.source_range.clone(),
            artifact_path: result.artifact_path.clone(),
            score: result.score,
            snippet: result.snippet.clone(),
        })
        .collect::<Vec<_>>();
    citations.sort_by(|left, right| {
        left.document_path
            .cmp(&right.document_path)
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    citations.dedup_by(|left, right| left.chunk_id == right.chunk_id);
    citations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{AiRuntime, Answer, Embeddings, ImageInput, ImageUnderstanding, Summary};
    use crate::store::DocumentRecord;
    use crate::workspace::Workspace;

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
                .citations
                .iter()
                .any(|citation| citation.document_path.ends_with("process.txt")
                    && citation.chunk_id == "chunk-1")
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
        assert!(answer.citations.is_empty());
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
        assert_eq!(answer.citations.len(), 1);
    }

    #[test]
    fn ai_runtime_answer_preserves_default_mock_behavior() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::init(dir.path()).unwrap();
        let store = MetadataStore::open(workspace.metadata_db_path()).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "runtime-process.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk(
                "chunk-runtime",
                "doc-1",
                "text",
                "运行时统一处理业务问答上下文。",
                None,
                None,
            )
            .unwrap();

        let runtime = AiRuntime::open(dir.path()).unwrap();
        let answer = runtime.answer("业务问答上下文怎么处理？").unwrap();

        assert!(answer.answer.contains("mock answer"));
        assert_eq!(answer.citations.len(), 1);
        assert_eq!(answer.citations[0].document_path, "runtime-process.txt");
        assert_eq!(answer.citations[0].chunk_id, "chunk-runtime");
    }
}
