## Why

第一批增强让 Word/PPT 可抽取、答案可引用、AI 调用可追踪，但图片内容仍只能单张描述，检索命中也缺少独立调试入口，报告仍偏资料概览。真实业务文档里流程图、截图和制度图表很多，需要把多模态描述显式写入索引，并让用户能先检查检索证据，再生成更有业务结构的报告。

本变更继续保持轻量和安全：图片 AI 入库必须显式开启，检索调试不调用 AI，业务分析优先使用本地索引和规则提取。

## What Changes

- 为 `ingest` 增加显式图片描述选项，将图片多模态描述作为 AI 生成 chunk 写入索引；未开启时仍只登记图片，不外呼 AI。
- 支持图片描述 dry-run，只记录 AI 调用计划和 trace，不写入图片描述 chunk。
- 增加 `search` CLI 命令，直接查看本地 FTS 命中的 chunk、score、页码/幻灯片、snippet 和来源，不调用 AI。
- 增强报告生成，提取业务对象、流程候选、规则/约束、风险/待确认问题，并带引用来源。
- 更新权限策略、测试和中文文档，确保新增命令和图片 AI 入库路径仍经过统一权限网关。

## Capabilities

### New Capabilities

- `retrieval-inspection`: 本地检索调试能力，覆盖不调用 AI 的 search 命令和可解释检索输出。
- `business-analysis-report`: 本地业务分析报告能力，覆盖业务对象、流程、规则、风险和来源引用提取。

### Modified Capabilities

- `document-extraction`: 增加显式图片多模态描述入库行为。
- `execution-permissions`: 增加 `search` 命令和图片 AI 入库选项的权限声明。

## Impact

- 代码：`src/main.rs`、`src/ingest/*`、`src/ai/runtime.rs`、`src/store.rs`、`src/report.rs`、`src/task.rs`、`tests/cli_flow.rs`。
- 数据：新增图片 AI 描述 chunk，使用现有 `chunks.ai_generated`、`kind`、`artifact_path`、`confidence` 字段。
- CLI：`ingest` 增加图片描述相关选项；新增 `search` 命令。
- 安全：图片描述入库必须显式开启；dry-run 不写描述 chunk，不发送图片正文。
- 文档：README、操作手册、数据文档、架构文档和 OpenSpec specs 同步更新。
