# answer-citations Specification

## Purpose
TBD - created by archiving change enhance-document-extraction-citations-observability. Update Purpose after archive.
## Requirements
### Requirement: 问答返回结构化引用
系统 SHALL 在问答结果中返回结构化引用，而不仅是来源文件路径。

#### Scenario: 有检索命中时返回 chunk 引用
- **WHEN** `ask` 命中一个或多个 chunk
- **THEN** 系统 MUST 返回包含 `chunk_id`、`document_path`、`page`、`slide`、`source_range` 和 `score` 的引用。

#### Scenario: 无检索命中时不调用 AI
- **WHEN** 本地索引没有命中任何 chunk
- **THEN** 系统 MUST 返回空引用列表，并且 MUST NOT 调用 AI provider。

### Requirement: CLI 输出可定位来源
系统 SHALL 在 CLI `ask` 输出中展示可定位来源，方便用户回到业务资料核对答案。

#### Scenario: CLI 展示引用元数据
- **WHEN** 用户执行 `ask` 且存在来源
- **THEN** CLI MUST 输出文件路径、chunk id、score，以及存在时的页码或幻灯片编号。

### Requirement: 搜索结果保持有界和可排序
系统 SHALL 使用配置的 `performance.context_chunks` 控制问答上下文数量，并按检索分数返回最相关 chunk。

#### Scenario: top-k 配置限制引用数量
- **WHEN** `performance.context_chunks = 3`
- **THEN** `ask` MUST 至多返回 3 个用于 AI 上下文的引用。

