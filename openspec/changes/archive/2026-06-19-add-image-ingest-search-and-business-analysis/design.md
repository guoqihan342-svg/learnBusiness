## Context

learnBusiness 当前可以索引文本、PDF、Word、PPT，并可通过 `describe-image` 单独调用多模态 AI 描述图片。缺口在于：图片描述不能作为 chunk 回到索引；用户缺少不调用 AI 的检索调试命令；报告仍主要展示资料概览，业务对象、流程、规则和风险提炼不足。

本次变更继续沿用本地优先和显式外呼原则。默认 `ingest` 不发送图片；只有用户显式传入图片描述选项时，才通过 `AiRuntime` 调用 provider。检索调试和报告分析都基于本地 SQLite/FTS 结果，不额外调用 AI。

## Goals / Non-Goals

**Goals:**

- 允许 `ingest --describe-images` 将图片多模态描述写成 AI 生成 chunk，并可被后续 `ask`、`search` 和 `report` 使用。
- 允许 `ingest --describe-images --dry-run-ai` 只记录调用计划和 trace，不写入描述 chunk。
- 增加 `search` CLI，本地展示 chunk、score、页码/幻灯片、snippet 和来源，帮助调试索引质量。
- 增强报告生成，通过轻量规则提取业务对象、流程候选、规则/约束、风险/待确认问题和来源引用。
- 将新增命令和图片 AI 入库路径纳入权限策略。

**Non-Goals:**

- 不实现完整 OCR、图像版面解析或扫描 PDF 识别。
- 不引入向量数据库或后台服务。
- 不让默认导入自动外呼 AI。
- 不把规则提取结果当作事实，只作为带来源的候选线索。

## Decisions

### 图片描述作为 AI 生成 chunk

`run_ingest` 增加 `IngestOptions`。当 `describe_images = true` 且文档为图片时，调用 `AiRuntime::describe_image`。非 dry-run 成功后写入 `kind = image` 或等价图片描述 chunk，设置 `ai_generated = true`、`artifact_path = 原图片路径`、`confidence = 80`，并继续使用原有 `ai_calls` 和 trace 审计。

这样做复用现有 AI runtime 的脱敏、审计、缓存和 trace，不让 ingest 绕过安全边界。

### Search 命令只走本地索引

新增 `search --workspace <path> <query> [--limit N]`。它直接调用 `MetadataStore::search_text`，输出 score、chunk、文件、页码/幻灯片和 snippet。该命令不构造 `AiRuntime`，不调用 provider。

这样用户能先判断索引和关键词是否命中，再决定是否执行 `ask`。

### 报告使用轻量规则提取

报告继续从 `MetadataStore::list_chunks` 读取本地 chunk。新增规则提取函数，按关键词和简单分隔符识别：

- 业务对象：包含“客户、订单、合同、申请、用户、系统、角色、部门”等词的短语。
- 流程候选：包含“流程、步骤、提交、审核、归档、审批、处理”等词的句子。
- 规则/约束：包含“必须、不得、需要、应当、规则、条件、限制”等词的句子。
- 风险/待确认：包含“风险、异常、失败、缺失、待确认、问题”等词的句子。

输出始终带来源路径和 chunk id，避免把启发式结果伪装成最终事实。

## Risks / Trade-offs

- [Risk] 图片描述可能包含模型误读。→ Mitigation：标记 `ai_generated = true`，报告和引用保留来源图片路径。
- [Risk] 图片导入外呼 AI 增加成本和泄漏面。→ Mitigation：必须显式传 `--describe-images`；dry-run 不发送图片正文。
- [Risk] 规则提取不如领域 skill 准确。→ Mitigation：定位为候选线索，并带来源；后续可接 skill 做领域模板。
- [Risk] search 命令输出太多影响可读性。→ Mitigation：默认 limit 使用配置 top-k 或小默认值，允许 `--limit` 控制。

## Migration Plan

1. 新增 `IngestOptions`，保留现有 `run_ingest(workspace, docs)` 默认行为不变。
2. 新图片描述 chunk 使用现有 `chunks` 字段，无需新增表。
3. 新 `search` 命令只读已有索引，旧工作区可直接使用。
4. 报告增强只改变 Markdown 输出内容，不改变存储格式。
