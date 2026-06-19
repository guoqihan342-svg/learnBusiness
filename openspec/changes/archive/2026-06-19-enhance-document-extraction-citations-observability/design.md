## Context

learnBusiness 当前已经具备本地工作区、文档发现、文本/PDF 抽取、SQLite FTS5 索引、问答、AI runtime 审计、trace 和可配置 HTTP provider。真实业务资料通常包含 Word、PPT、图片和扫描件；如果这些内容不能进入索引，agent 对业务的理解会明显受限。

当前代码中 `Chunk` 已经预留 `page`、`slide`、`source_range`、`artifact_path`、`confidence`、`ai_generated` 字段，`Citation` 模型也已经存在，但 `store.insert_chunk` 和 `qa` 尚未完整使用这些元数据。`task.rs` 中已有权限模型，但 CLI 命令尚未统一声明和校验权限。

本设计以轻量实现为第一原则：先补 Office Open XML 正文抽取、细粒度引用、trace 关联和权限网关，不引入常驻服务，不把 OCR、向量数据库和复杂业务建模塞进本次变更。

## Goals / Non-Goals

**Goals:**

- 让 `.docx` 和 `.pptx` 的正文进入本地索引，提升真实业务资料的可问答覆盖率。
- 在搜索结果、问答返回值和 CLI 输出中暴露 chunk 级引用，包含文件、chunk、页码/幻灯片、分数和片段。
- 为 AI 调用记录增加 trace 关联，使 `inspect-ai` 能帮助定位对应 trace 日志。
- 将权限模型接入 CLI 命令入口，形成读本地、写工作区、外部 AI、外部网络和后续 MCP 的统一校验路径。
- 保持默认离线、轻量、省 token、安全日志的现有约束。

**Non-Goals:**

- 不实现完整 OCR、版面分析或扫描 PDF 图像识别。
- 不引入独立向量数据库、后台服务或队列系统。
- 不改变默认 `mock` provider，也不绑定任何特定 AI 厂商。
- 不把 skill/MCP 的具体执行器做完整，只保留权限边界和后续接入位置。

## Decisions

### 使用 Office Open XML 轻量抽取

`.docx` 和 `.pptx` 本质是 zip 包中的 XML。实现上新增 `office` 抽取路径：读取 `word/document.xml`、`ppt/slides/slide*.xml` 等条目，用 XML reader 提取文本节点，按段落、表格单元和幻灯片合并成文本块。

选择理由：这比 shell 调用 Office/LibreOffice 更轻、更可移植，也比先接 AI/OCR 更省 token、更安全。缺点是复杂布局、嵌入图片和备注信息可能抽不全，后续可通过 artifact + AI/OCR 增强。

### 使用 chunk 元数据承载引用

`chunks` 表已有 `page`、`slide`、`source_range`、`artifact_path` 等字段，本次优先复用这些字段，不新建复杂引用表。`SearchResult` 扩展为携带这些元数据，`QaAnswer` 从 `Vec<String>` 来源升级为 `Vec<Citation>`。

选择理由：引用和 chunk 生命周期绑定，复用现有表能保持轻量；后续如果需要多证据、多页面范围，再从 `Citation` 模型演进。

### trace_id 写入 AI 审计

AI runtime 目前会生成 trace id 并写入 `trace.jsonl`，但 `ai_calls` 记录没有保存该 id。新增兼容迁移列 `trace_id`，在成功、失败和 dry-run 审计中写入；`inspect-ai` 默认输出 trace id，并支持按 trace id 过滤。

选择理由：排障时从 CLI 审计跳到 trace 日志是最短路径。只保存 id 和元数据，不保存 prompt、业务正文或请求头值。

### CLI 入口声明权限

新增 `CommandPermissionPolicy` 或等价 helper，为每个 CLI 命令声明所需权限。默认本地命令只需要 `ReadLocal`/`WriteWorkspace`，真实外部 HTTP AI 需要 `AiExternal` 和 `ExternalNetwork`，dry-run 不需要外部网络权限。

选择理由：权限控制要在命令入口统一，避免后续 skill/MCP 或 provider 分支绕过安全边界。第一版可以使用内置授权集合，后续再从配置文件或任务描述中加载更细的授权策略。

## Risks / Trade-offs

- [Risk] Office XML 轻量抽取无法还原复杂版面、图片和 SmartArt。→ Mitigation：把能力定位为正文抽取，保留 artifact 和 `needs_ai` 语义，为后续 OCR/多模态补全留入口。
- [Risk] `QaAnswer.sources` 类型变化会影响测试和 CLI 输出。→ Mitigation：集中修改调用点，保持 CLI 文本输出兼容人类阅读，库层返回结构化引用。
- [Risk] SQLite 迁移可能遇到旧工作区缺列。→ Mitigation：继续使用启动时 `PRAGMA table_info` 检查并增量 `ALTER TABLE`。
- [Risk] 权限网关如果一次做太复杂会拖慢开发。→ Mitigation：先实现命令级声明和校验，保留策略加载扩展点，不引入复杂 RBAC。
- [Risk] 新依赖增加构建体积。→ Mitigation：只选择 zip/xml 解析所需轻量依赖，避免引入 Office 自动化或大型文档处理运行时。

## Migration Plan

1. 更新 SQLite 打开逻辑，自动补齐 `chunks` 和 `ai_calls` 的新增/既有元数据列。
2. 保持旧工作区可打开；旧 chunk 没有页码、幻灯片或 trace id 时输出 `-`。
3. 新导入的 `.docx` 和 `.pptx` 开始写入正文 chunk；旧工作区需要重新执行 `ingest` 才会获得这些内容。
4. 如果新抽取依赖出现问题，可回退到上一提交；工作区数据只新增元数据列，不破坏旧表。

## Open Questions

- 完整 OCR、扫描 PDF 和图片多模态入库应作为下一批 OpenSpec 变更处理。
- 向量检索是否默认关闭、只作为可选重排，也应在后续检索增强变更中单独设计。
