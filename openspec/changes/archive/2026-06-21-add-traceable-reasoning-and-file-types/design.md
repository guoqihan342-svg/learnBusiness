## Context

learnBusiness 当前已经具备本地发现、抽取、切块、SQLite FTS5 检索、AI provider 网关、AI 审计和 trace 日志。现有 trace 主要覆盖 `AiRuntime` provider 调用；`ingest`、`search`、`ask` 的业务步骤仍主要依赖 CLI 输出和测试推断，缺少同一 trace id 下的步骤级证据链。

本变更的约束是：安全优先、轻量、省 token、可定位问题。推算过程必须是外部可验证步骤摘要，而不是模型内部思维链；日志不得保存完整 prompt、业务正文、图片 base64、请求头值或 API key。

## Goals / Non-Goals

**Goals:**
- 给主命令路径增加结构化步骤日志，覆盖开始、检索、选择、截断、脱敏、AI 调用、引用绑定、完成或失败。
- 让 `ask` 返回并输出安全的推算过程摘要，便于用户理解答案来自哪些本地步骤。
- 新增本地 trace inspection 命令，按 trace id 或最近事件查看步骤日志。
- 扩展轻量文件类型识别与抽取：CSV、TSV、JSON、HTML、XML、YAML/YML、XLSX。
- 不增加额外 AI 调用，不扩大默认上下文，不保存敏感正文。

**Non-Goals:**
- 不实现完整 OCR、扫描 PDF 版面分析或复杂 Office 公式/样式还原。
- 不引入独立日志服务、消息队列或后台守护进程。
- 不输出模型隐藏推理链，只输出本地可验证的步骤、数量、hash、状态和来源引用。
- 不把 `context_chunks` 自动调高；更多业务理解优先靠索引质量和来源追踪。

## Decisions

1. **新增通用 OperationTrace，而不是扩展 AI trace 字段。**  
   `AiRuntime` 的 trace 专注 provider 调用，步骤日志覆盖命令级流程。二者通过同一 trace id 关联。这样避免把 ingest/search 的本地步骤伪装成 AI 调用，也避免污染 `ai_calls` 审计语义。

2. **步骤日志写 JSONL 文件，不写入 SQLite。**  
   JSONL 适合追加写、排障和按 trace id 过滤，不需要迁移表结构。SQLite 继续保存可查询的文档、chunk 和 AI 审计。后续如果需要 UI 查询，再考虑索引化。

3. **推算过程摘要来自运行元数据。**  
   `QaAnswer` 增加 `reasoning_steps`，内容包括本地检索命中数、选取 chunk 数、单 chunk 截断上限、是否脱敏、token 估算、AI 调用状态、引用数量和 trace id。摘要不包含完整 chunk 正文，也不额外调用 AI。

4. **新增文件类型先采用保守文本抽取。**  
   CSV/TSV 直接读取文本；JSON/YAML/XML/HTML 提取文本值或去标签文本；XLSX 读取 `sharedStrings.xml` 和 worksheet inline string，按工作表生成 table chunk。这样覆盖常见业务资料，同时保持依赖少、速度快。

5. **CLI 默认展示安全推算过程。**  
   `ask` 输出新增“推算过程”块，便于立即排障。`inspect-trace` 提供完整步骤事件查看。输出不展示正文原文，来源正文仍通过已有引用 snippet 控制。

## Risks / Trade-offs

- [Risk] 步骤日志数量增加导致文件膨胀。  
  Mitigation: 每个命令只记录关键步骤，字段保持短小，不记录正文。

- [Risk] 用户把“推算过程”理解为模型内部思考。  
  Mitigation: 文档和 CLI 明确这是本地证据链摘要，不是隐藏思维链。

- [Risk] HTML/XML/YAML 轻量抽取不能完全理解复杂结构。  
  Mitigation: 作为业务检索入口先保留可读文本，复杂解析留给后续 provider/skill/MCP 扩展。

- [Risk] XLSX 单元格关系复杂，简单抽取可能丢失格式和公式结果。  
  Mitigation: 当前目标是可检索业务文本，先抽取共享字符串、inline 字符串和数值；不承诺公式计算。

## Migration Plan

- 新版本启动后自动创建 `.learnBusiness/logs/operations.jsonl` 所在目录。
- 已有 workspace 不需要数据库迁移。
- `ask` CLI 输出会多出“推算过程”段落，原有答案和来源输出保持。
- 如果需要回滚，删除新增日志文件不影响索引和 AI 审计。

## Open Questions

无。实现阶段如发现 OpenSpec 对新 CLI 命名有更合适约定，优先保持用户可理解性和测试可验证性。
