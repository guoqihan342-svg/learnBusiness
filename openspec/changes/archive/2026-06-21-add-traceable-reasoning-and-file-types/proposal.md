## Why

当前版本已经能把业务文档索引、检索并交给 AI 回答，但“为什么这样回答”的外部推算过程还不够显式：用户只能看到来源引用和 AI 审计，难以定位检索、截断、脱敏、调用和引用绑定中的具体步骤。与此同时，业务资料经常包含表格、配置、网页导出和结构化数据文件，现有文件类型覆盖还偏少。

本变更要在不泄漏原文、不增加额外 AI token 的前提下，把 agent 的业务理解过程做成可追踪、可解释、可测试的本地证据链，并扩展轻量文件类型识别与抽取。

## What Changes

- 新增结构化步骤日志：记录命令、组件、步骤、状态、trace id、输入/输出 hash、命中数量、token 估算、错误分类和耗时，不保存完整 prompt、业务正文、图片 base64、请求头值或 API key。
- 问答结果新增“推算过程摘要”：展示本地检索、top-k 选择、上下文截断、脱敏判断、AI 调用和引用绑定等可验证步骤。
- `ask` CLI 默认输出安全的推算过程摘要，便于问题定位；摘要只使用元数据、hash、数量和引用编号，不额外调用 AI。
- 新增本地 `inspect-trace` 类能力或等价 CLI，用于按 trace id 查看步骤级日志，覆盖 ingest、search、ask、describe-image 等主路径。
- 扩展文档识别和抽取：新增 CSV、TSV、JSON、HTML、XML、YAML/YML 和 XLSX 的轻量文本抽取与索引。
- 保持低 token 策略：新增推算过程和日志都来自本地运行元数据，不扩大 `performance.context_chunks`，不把更多正文发给 provider。

## Capabilities

### New Capabilities
- `operation-step-tracing`: 约束命令级和组件级步骤日志、trace 查询、安全字段边界和失败可定位行为。

### Modified Capabilities
- `answer-citations`: 问答结果需要返回安全的推算过程摘要，并在 CLI 中展示。
- `document-extraction`: 增加 CSV、TSV、JSON、HTML、XML、YAML/YML 和 XLSX 的识别与抽取要求。
- `retrieval-inspection`: 本地 search 需要写入安全步骤日志，便于确认检索过程且不触发 AI。
- `ai-provider-runtime`: AI runtime 的 trace 需要和外层操作 trace 对齐，避免 AI 调用步骤与命令步骤断裂。

## Impact

- 影响模块：`src/main.rs`、`src/trace.rs`、`src/workspace.rs`、`src/qa.rs`、`src/ai/runtime.rs`、`src/ingest/*`、`src/discover.rs`、`src/store.rs`。
- 影响 CLI：`ask` 输出新增推算过程；新增或扩展 trace inspection 命令。
- 影响数据：新增本地 JSONL 步骤日志文件；不改变已有 SQLite 核心表结构，除非实现证明必须增加字段。
- 影响测试：新增 RED/GREEN 覆盖文件类型识别/抽取、问答推算摘要、步骤日志安全边界、trace 查询和不额外调用 AI。
