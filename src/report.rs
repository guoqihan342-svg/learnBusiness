use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::store::MetadataStore;
use crate::workspace::Workspace;

pub struct ReportGenerator;

impl ReportGenerator {
    pub fn generate(store: &MetadataStore) -> Result<String> {
        let document_count = store.document_count()?;
        let chunks = store.list_chunks(20)?;
        let source_lines = chunks
            .iter()
            .map(|chunk| {
                format!(
                    "- `{}`: {}",
                    chunk.document_path,
                    first_line(&chunk.snippet)
                )
            })
            .collect::<Vec<_>>();

        Ok(format!(
            "\
# 业务理解报告

## 执行摘要

当前资料集包含 {document_count} 个已登记文档。第一版报告基于本地索引生成，重点展示已抽取文本和来源。

## 资料集概览

已索引内容块数量：{}

## 核心业务对象

- 待从更多文档和领域 skill 中提炼。

## 主要业务流程

{}

## 需要确认的问题

- 哪些规则属于强约束，哪些只是当前材料中的描述？
- 文档中的流程是否覆盖异常路径和人工处理路径？

## 来源引用

{}
",
            chunks.len(),
            summarize_flow_candidates(&chunks),
            if source_lines.is_empty() {
                "- 暂无来源。".to_string()
            } else {
                source_lines.join("\n")
            }
        ))
    }
}

pub fn report_workspace(workspace_root: impl AsRef<Path>, out: impl AsRef<Path>) -> Result<()> {
    let workspace = Workspace::open(workspace_root);
    let store = MetadataStore::open(workspace.metadata_db_path())?;
    let report = ReportGenerator::generate(&store)?;
    fs::write(out, report)?;
    Ok(())
}

fn summarize_flow_candidates(chunks: &[crate::store::SearchResult]) -> String {
    let lines = chunks
        .iter()
        .filter(|chunk| chunk.snippet.contains("流程") || chunk.snippet.contains("规则"))
        .map(|chunk| format!("- {}", first_line(&chunk.snippet)))
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "- 当前索引中还没有足够信息生成流程摘要。".to_string()
    } else {
        lines.join("\n")
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(120)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::DocumentRecord;

    #[test]
    fn report_contains_required_sections() {
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

        let report = ReportGenerator::generate(&store).unwrap();
        assert!(report.contains("## 执行摘要"));
        assert!(report.contains("## 来源引用"));
        assert!(report.contains("process.txt"));
    }
}
