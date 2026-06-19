## Why

learnBusiness 已经具备本地索引、可配置 HTTP AI provider、审计和 trace 基础，但真实业务资料落地时还存在三个明显缺口：Word/PPT 内容不能进入索引、问答引用只到文件级别、权限和排障信息尚未形成 CLI 级闭环。

本变更聚焦让工具更接近真实业务文档场景：在保持轻量、省 token、安全的前提下，提高可问答内容覆盖率、答案可信度和问题定位效率。

## What Changes

- 为 `.docx` 和 `.pptx` 增加轻量正文抽取，提取可索引文本并保留基础来源位置。
- 增强 chunk 元数据和搜索结果，问答输出从“来源文件”升级为“文件 + chunk + 页码/幻灯片 + 分数”的细粒度引用。
- 为 `inspect-ai` 和 AI runtime 输出增加 trace 标识，方便从一次失败定位到 `.learnBusiness/logs/trace.jsonl` 中的结构化记录。
- 将现有权限模型接入 CLI 命令路径，对本地读、工作区写、外部 AI、外部网络和后续 MCP 能力形成统一校验入口。
- 补充测试语料、中文文档和操作说明，确保新增能力可验证、可排障、可安全回退。
- 不引入常驻服务或重型向量数据库；向量检索、完整 OCR 和复杂业务建模继续作为后续增强。

## Capabilities

### New Capabilities

- `document-extraction`: 业务文档正文抽取和索引能力，覆盖文本、Markdown、PDF、Word、PPT 和待 AI/OCR 补全资产的登记行为。
- `answer-citations`: 问答结果引用能力，要求答案携带可审计、可定位的来源元数据。
- `execution-permissions`: CLI 执行权限能力，要求敏感命令经过统一权限声明和校验。

### Modified Capabilities

- `ai-provider-runtime`: 增强 AI 调用 trace 标识、失败诊断和 CLI 审计查看行为。

## Impact

- 代码：`src/ingest/*`、`src/store.rs`、`src/models.rs`、`src/qa.rs`、`src/ai/runtime.rs`、`src/main.rs`、`src/task.rs`、`src/trace.rs`。
- 数据：`chunks` 表已有 `page`、`slide`、`source_range`、`artifact_path` 字段，可能需要补充写入和兼容迁移；`ai_calls` 可能需要增加 trace 相关字段。
- 依赖：需要增加轻量 Office Open XML 解析依赖，用于读取 `.docx` 和 `.pptx` zip/xml 内容。
- CLI：`ask`、`ingest`、`inspect-ai`、`describe-image` 的输出和内部权限校验会增强，但不改变现有基础用法。
- 文档：README、操作手册、数据文档、架构文档和 OpenSpec 需要同步更新。
