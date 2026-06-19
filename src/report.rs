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
                    "- `{}` chunk={}: {}",
                    chunk.document_path,
                    chunk.chunk_id,
                    first_line(&chunk.snippet)
                )
            })
            .collect::<Vec<_>>();
        let business_objects = extract_business_objects(&chunks);
        let flow_candidates = collect_sentences(
            &chunks,
            &["流程", "步骤", "提交", "审核", "审批", "归档", "处理"],
        );
        let rule_candidates = collect_sentences(
            &chunks,
            &["规则", "必须", "不得", "需要", "应当", "条件", "限制"],
        );
        let risk_candidates =
            collect_sentences(&chunks, &["风险", "异常", "失败", "缺失", "待确认", "问题"]);

        Ok(format!(
            "\
# 业务理解报告

## 执行摘要

当前资料集包含 {document_count} 个已登记文档。第一版报告基于本地索引生成，重点展示已抽取文本和来源。

## 资料集概览

已索引内容块数量：{}

## 核心业务对象

以下为本地规则提取的候选对象，需要业务负责人复核：

{}

## 主要业务流程

以下为本地索引中的流程候选线索：

{}

## 规则与约束

{}

## 风险与待确认

{}

## 需要确认的问题

- 哪些规则属于强约束，哪些只是当前材料中的描述？
- 文档中的流程是否覆盖异常路径和人工处理路径？

## 来源引用

{}
",
            chunks.len(),
            format_lines_or_empty(&business_objects, "- 暂无候选对象。"),
            format_lines_or_empty(&flow_candidates, "- 当前索引中还没有足够信息生成流程摘要。"),
            format_lines_or_empty(&rule_candidates, "- 暂无规则或约束候选。"),
            format_lines_or_empty(&risk_candidates, "- 暂无风险或待确认候选。"),
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

fn extract_business_objects(chunks: &[crate::store::SearchResult]) -> Vec<String> {
    let keywords = [
        "客户", "订单", "合同", "申请", "用户", "系统", "角色", "部门", "运营", "审批",
    ];
    let mut lines = Vec::new();
    for chunk in chunks {
        for keyword in keywords {
            if chunk.snippet.contains(keyword) {
                lines.push(format!("- {keyword} ({})", source_label(chunk)));
            }
        }
    }
    lines.sort();
    lines.dedup();
    lines
}

fn collect_sentences(chunks: &[crate::store::SearchResult], keywords: &[&str]) -> Vec<String> {
    let mut lines = Vec::new();
    for chunk in chunks {
        for sentence in split_sentences(&chunk.snippet) {
            if keywords.iter().any(|keyword| sentence.contains(keyword)) {
                lines.push(format!(
                    "- {} ({})",
                    first_line(sentence),
                    source_label(chunk)
                ));
            }
        }
    }
    lines.sort();
    lines.dedup();
    lines
}

fn split_sentences(text: &str) -> Vec<&str> {
    text.split(['。', '！', '？', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

fn source_label(chunk: &crate::store::SearchResult) -> String {
    let mut parts = vec![
        format!("`{}`", chunk.document_path),
        format!("chunk={}", chunk.chunk_id),
    ];
    if let Some(page) = chunk.page {
        parts.push(format!("page={page}"));
    }
    if let Some(slide) = chunk.slide {
        parts.push(format!("slide={slide}"));
    }
    parts.join(" ")
}

fn format_lines_or_empty(lines: &[String], empty: &str) -> String {
    if lines.is_empty() {
        empty.to_string()
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

    #[test]
    fn report_extracts_business_candidates_with_sources() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path().join("metadata.sqlite")).unwrap();
        let doc = DocumentRecord::new_for_test("doc-1", "policy.txt", "text/plain");
        store.upsert_document(&doc).unwrap();
        store
            .insert_chunk(
                "chunk-1",
                "doc-1",
                "text",
                "客户提交合同申请，运营部门必须审核。异常风险需要待确认。核心流程是提交、审核、归档。",
                None,
                None,
            )
            .unwrap();

        let report = ReportGenerator::generate(&store).unwrap();
        assert!(report.contains("## 核心业务对象"));
        assert!(report.contains("客户"));
        assert!(report.contains("合同"));
        assert!(report.contains("## 规则与约束"));
        assert!(report.contains("必须审核"));
        assert!(report.contains("## 风险与待确认"));
        assert!(report.contains("异常风险"));
        assert!(report.contains("chunk-1"));
    }
}
