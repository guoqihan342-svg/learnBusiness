# 业务文档理解 Agent 设计

## 背景

这个 agent 用来理解业务文档，而不是理解代码仓库。输入是一批本地业务资料，可能包含 PDF、Word、PowerPoint、图片、扫描页、流程图、表格、截图和混合 Office 导出文件。

系统必须省 token、默认安全、适合重复本地运行，并且可以扩展 AI provider、skill 和 MCP server。实现语言使用 Rust。

我们参考了 `mainframecomputer/orchestra`。它值得借鉴的是 `Task / Agent / Tool` 抽象、以任务为中心的编排、显式任务依赖、MCP adapter 形态、迭代上限、重复 tool call 检测、模型 fallback 和事件 callback。它不是文档处理框架，所以本设计只参考它的编排思想，不照搬它的文档解析方式。

参考项目：

- https://github.com/mainframecomputer/orchestra
- https://github.com/mainframecomputer/orchestra/blob/main/packages/python/src/mainframe_orchestra/task.py
- https://github.com/mainframecomputer/orchestra/blob/main/packages/python/src/mainframe_orchestra/orchestration.py
- https://github.com/mainframecomputer/orchestra/blob/main/packages/python/src/mainframe_orchestra/adapters/mcp_adapter.py

## 目标

- 读取一个本地业务文档目录。
- 把文本、表格、图片、页面、幻灯片和 AI 生成的图片说明抽取成可追溯的内容块。
- 建立本地索引，支持关键词搜索、语义搜索、来源引用、报告生成和问答。
- 只在有价值时调用 AI：图片理解、扫描页理解、复杂表格解释、摘要、报告生成和基于索引的问答。
- 用稳定内容 hash 缓存 AI 调用，避免重复花 token。
- 通过 provider trait 支持 OpenAI-compatible 多模态和 embedding API。
- 为 MCP 工具和业务领域 skill 预留扩展点。
- 默认让原始文件留在本地，除非某次配置过的 AI 或 MCP 调用明确需要发送选中的内容。

## 第一版不做什么

- 不做完全自主的多 agent 自由规划循环。
- 不强制依赖外部向量数据库。
- 不做完整业务知识图谱。
- 不做 GUI。
- 不接云端文档同步，除非以后通过 MCP server 提供。
- 不修改原始业务文档。

## 用户工作流

```text
biz-agent init <workspace>
biz-agent ingest <docs_dir> --workspace <workspace>
biz-agent status --workspace <workspace>
biz-agent inspect-ai --workspace <workspace>
biz-agent report --workspace <workspace> --out report.md
biz-agent ask --workspace <workspace> "这个业务的核心流程是什么？"
```

`init` 创建本地工作区。`ingest` 解析文档并建立索引。`inspect-ai` 展示待执行或已执行的 AI 调用，包括调用原因、hash、预估 token 和脱敏状态。`report` 从索引生成业务理解报告。`ask` 基于索引问答，并引用到文件、页码、幻灯片或抽取出来的图片。

## 总体架构

```text
业务文档目录
  -> 文件发现和 hash 扫描
  -> 按格式抽取内容
  -> 归一化内容块
  -> 本地元数据索引、全文索引、向量索引
  -> 选择性 AI 增强
  -> 报告生成和 RAG 问答
```

Rust 负责调度、解析、缓存、索引、安全检查和 CLI。AI provider 是工具，不是事实来源。事实来源是本地归一化内容块和索引。

## 工作区布局

```text
.agent-index/
  config.toml
  metadata.sqlite
  fulltext/
  vectors/
  artifacts/
    images/
    pages/
    thumbnails/
  cache/
    ai/
    extraction/
  logs/
    ingest.jsonl
    ai.jsonl
```

`metadata.sqlite` 保存 documents、chunks、tasks、AI 调用记录和引用信息。`fulltext` 保存全文搜索状态。`vectors` 保存 embedding 和向量元数据。`artifacts` 保存抽取出的页面图片、文档内嵌图片，以及必要时渲染出的页面或幻灯片快照。

## 数据模型

```text
Document
  id
  path
  file_type
  content_hash
  modified_at
  size_bytes
  ingest_status

Chunk
  id
  document_id
  kind: text | table | image | page | slide | ai_summary | ocr_text
  text
  page
  slide
  source_range
  artifact_path
  confidence
  ai_generated
  content_hash

AiCall
  id
  task_id
  provider
  model
  purpose
  input_hash
  output_hash
  token_estimate
  redaction_applied
  status

Citation
  chunk_id
  document_path
  page
  slide
  source_range
```

报告里的每个关键结论和问答里的每个答案都应该能引用一个或多个 `Chunk`。AI 生成的 chunk 必须显式标记，方便下游区分原始资料和模型推断。

## 索引策略

第一版建立三层索引：

- SQLite 元数据索引：保存文档、内容块、hash、任务状态和引用信息。
- 全文索引：搜索抽取文本、OCR 文本、表格文本和 AI 生成的图片说明。
- 向量索引：对文本块、图片说明、页面摘要、幻灯片摘要和文档摘要做语义检索。

`ask` 的检索路径采用混合检索：

```text
问题
  -> 关键词候选
  -> 向量候选
  -> 元数据过滤和去重
  -> top-k 上下文包
  -> AI 生成带引用答案
```

这样不会只依赖向量搜索，答案也更容易追溯来源。

## 文档抽取

PDF 处理：

- 优先抽取 PDF 内嵌文本。
- 如果页面没有文本、置信度低，或包含重要图示，则把选中的页面渲染为图片。
- 能抽取图片时，把图片挂到对应页面级 chunk 上。

Word 处理：

- 抽取段落、标题、表格和内嵌图片。
- 尽量保留文档顺序和章节层级。

PowerPoint 处理：

- 抽取幻灯片文本、备注、表格和内嵌图片。
- 对包含图示、图表或文本很少的幻灯片生成快照，交给多模态模型分析。

图片处理：

- 保存原始图片 artifact。
- 只有当图片被文档引用、可能包含业务含义，或用户明确请求时，才生成 AI 图片说明。

解析层优先使用 Rust 库，必要时再调用外部转换器。外部工具需要在启动或 `status` 中做能力检测，并给出清晰诊断。

## AI 策略

AI 调用必须选择性执行，并且必须缓存。

适合使用 AI 的场景：

- 描述流程图、架构图、组织结构图、截图、扫描页和复杂表格。
- 把 chunk 组合摘要成页面摘要、文档摘要和资料集摘要。
- 生成最终业务理解报告。
- 基于检索到的上下文回答问题。

不适合使用 AI 的场景：

- 本地可以完成的普通文本抽取。
- 重复处理没有变化的文件。
- 只需要少量 chunk 时发送整份文档。

Provider 接口：

```text
AiProvider
  describe_image(image, prompt) -> ImageUnderstanding
  summarize_chunks(chunks, prompt) -> Summary
  embed_texts(texts) -> Embeddings
  answer(question, contexts) -> Answer
```

第一版 provider 面向 OpenAI-compatible HTTP API，支持配置 `base_url`、`api_key`、模型名、超时和重试策略。接口要允许后续接 Azure OpenAI、本地模型网关和企业 AI 网关。

## Token 节省策略

- 文件和 chunk 使用稳定 hash，未变化内容跳过。
- AI 缓存 key 包含 provider、model、purpose、prompt version、content hash 和 redaction mode。
- Ingest 阶段先本地解析，再选择性 AI 增强。
- 问答只发送 top-k 相关 chunk。
- 报告生成使用分层摘要，不把整个资料集直接塞进上下文。
- 工具循环设置明确迭代上限。
- 大模型输出使用结构化 JSON schema，减少重试和解析歧义。

## 安全策略

默认 local-first。

- 原始文件保留在本地，除非某个被允许的 AI 或 MCP 工具调用需要选中的内容。
- `--dry-run-ai` 只展示将发送什么、为什么发送、预估大小，不真正调用外部 API。
- 本地脱敏可以在外部 AI 调用前 mask 手机号、邮箱、身份证号、银行卡样式数字和密钥样式文本。
- AI 日志默认只保存 hash、模型名、调用目的、token 估算、状态和脱敏状态，不保存完整敏感 prompt。
- MCP 工具默认关闭，必须配置后才可用。
- 每个工具都有明确权限类别：`read_local`、`write_workspace`、`external_network`、`ai_external`、`mcp_external`。
- 当工具请求未授权权限时，CLI 默认拒绝执行。

## 编排模型

设计借鉴 Orchestra 的任务中心模型，但在 Rust 中做成强类型，并且第一版默认确定性执行。

```text
Task
  id
  kind
  input_refs
  output_refs
  required_permissions
  token_budget
  max_iterations
  status

Agent
  id
  role
  goal
  allowed_tools
  model_policy

Tool
  name
  schema
  permission
  implementation: local | ai | mcp
```

第一版内置 agent：

- `ingest_agent`：文档发现、抽取、切块和 artifact 生成。
- `vision_agent`：图片、图示、扫描页和幻灯片快照理解。
- `index_agent`：元数据、全文和向量索引更新。
- `report_agent`：业务理解报告生成。
- `qa_agent`：基于检索增强的问答。

这些 agent 在第一版不是自由行动的角色，而是有边界的强类型执行单元，有明确输入和允许工具。

## MCP 和 Skill 扩展点

MCP 支持参考 Orchestra 的 adapter 形态：

- 连接 stdio 或 SSE MCP server。
- 列出可用工具。
- 把工具 schema 转换成本地强类型 tool descriptor。
- 调用前执行权限检查。
- 支持在工具调用中附加 workspace id、tenant id 或 trace id 等元数据。
- 可预测地关闭 session 和子进程。

Skill 是领域模板，不是执行信任边界。Skill 可以提供 prompt、报告章节、实体定义和领域问题清单，例如“保险业务”“供应链业务”“金融风控”。解析、索引、权限和 AI 调用仍然由引擎控制。

## 业务报告结构

第一版报告包含：

- 执行摘要。
- 资料集概览。
- 业务领域和范围。
- 关键角色和干系人。
- 核心业务对象。
- 主要业务流程。
- 业务规则和约束。
- 文档中提到的系统、表单、数据和集成点。
- 风险、歧义和矛盾。
- 需要业务专家确认的问题。
- 来源引用。

报告必须引用来源 chunk，并且清晰标记 AI 推断内容。

## 错误处理

- 单个文件失败只记录 warning，不中断整个 ingest。
- AI 调用失败时保留本地抽取结果，并把相关 chunk 标记为 `needs_ai`。
- 缺失外部转换器时由 `status` 报告，并给出安装建议。
- 损坏或加密文件跳过，并给出清晰诊断。
- AI 返回非法 JSON 时，在配置的重试上限内重试。
- 重复相同工具调用时停止循环并记录。
- 达到最大迭代数或 token budget 时输出部分结果，而不是静默失败。

## 测试策略

单元测试：

- 文件 hash。
- chunk id 稳定性。
- 脱敏。
- AI 缓存 key 生成。
- JSON schema 解析。
- 权限检查。
- 检索去重。

集成测试：

- 包含 PDF、DOCX、PPTX、PNG 和混合 artifact 的小型目录。
- 不调用 AI 的 ingest。
- 使用 mock AI provider 的 ingest。
- 重复 ingest 未变化文件，验证 cache hit。
- 提问并验证引用 chunk。
- 生成报告并验证必要章节。

性能检查：

- 大量小文件。
- 单个大 PDF。
- 包含大量图片的 PPTX。
- 轻微修改后的重复 ingest。

## 实现说明

Rust crate 布局保持模块化：

```text
src/
  main.rs
  config.rs
  workspace.rs
  discover.rs
  task.rs
  agent.rs
  tool.rs
  ingest/
  index/
  ai/
  mcp/
  report.rs
  qa.rs
```

具体 crate 选择在实现中通过编译验证，不在设计阶段过早锁死。候选包括 `clap`、`tokio`、`serde`、`reqwest`、`rusqlite`、`tantivy`、`pdf-extract`，以及一个小型本地向量索引抽象。

## 第一版实现切片

第一版先做一个可运行的纵向切片：

1. CLI 和工作区初始化。
2. 文件发现、hash 和 SQLite 元数据。
3. 纯文本和基础 PDF 文本抽取。
4. chunk 存储和全文搜索。
5. mock AI provider 和 AI 缓存。
6. 基于本地检索的 `ask`。
7. 配置驱动的 OpenAI-compatible provider。
8. 独立图片的描述入口。
9. 基础报告生成。

这样可以在深入 Office 解析和 MCP 集成之前，先得到一个可测试、可运行、可演进的版本。
