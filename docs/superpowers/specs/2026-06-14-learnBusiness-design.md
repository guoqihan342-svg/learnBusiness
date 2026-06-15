# learnBusiness 设计说明

## 背景

`learnBusiness` 用来理解本地业务资料，而不是理解代码仓库。输入可以是 PDF、Word、PowerPoint、图片、扫描页、流程图、表格、截图和混合 Office 导出文件。

系统目标是本地优先、轻量、省 token、默认安全，并且可以逐步扩展 AI provider、skill 和 MCP server。实现语言使用 Rust。

## 目标

- 读取一个本地业务文档目录。
- 把文本、图片、页面、幻灯片和 AI 生成的图片说明抽取成可追溯内容块。
- 建立本地索引，支持关键词搜索、来源引用、报告生成和问答。
- 只在有价值时调用 AI：图片理解、扫描页理解、复杂表格解释、摘要、报告生成和基于索引的问答。
- 用稳定内容 hash 缓存 AI 调用，避免重复花 token。
- 通过 `provider=http` 支持可配置 `base_url`、请求头、文本问答、embedding 和多模态接口。
- 为 MCP 工具和业务领域 skill 预留扩展点。
- 默认让原始文件留在本地，除非某次配置过的 AI 或 MCP 调用明确需要发送选中的内容。

## 用户工作流

```text
learnBusiness init <workspace>
learnBusiness ingest <docs_dir> --workspace <workspace>
learnBusiness status --workspace <workspace>
learnBusiness inspect-ai --workspace <workspace>
learnBusiness report --workspace <workspace> --out report.md
learnBusiness ask --workspace <workspace> "这个业务的核心流程是什么？"
learnBusiness describe-image <image_path> --workspace <workspace> --dry-run-ai
```

## 工作区布局

运行时目录统一命名为 `.learnBusiness/`。配置文件只放在 `config/` 文件夹里，索引、缓存、日志和 artifact 按用途隔离。

```text
.learnBusiness/
  config/
    app.toml
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
```

`.learnBusiness/` 不应提交到 Git。配置文件不保存真实密钥值，真实密钥通过环境变量或外部密钥管理提供。

## 架构

```text
业务文档目录
  -> 文件发现和 hash 扫描
  -> 按格式抽取内容
  -> 定长切分内容块
  -> 本地元数据索引和全文索引
  -> 选择性 AI 增强
  -> 报告生成和 RAG 问答
```

Rust 负责调度、解析、缓存、索引、安全检查和 CLI。AI provider 是工具，不是事实来源。事实来源是本地归一化内容块和索引。

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
  kind
  text
  page
  slide
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
```

报告中的关键结论和问答答案都应能引用一个或多个 `Chunk`。AI 生成内容必须显式标记，方便下游区分原始资料和模型推断。

## 省 Token 策略

- 文件和 chunk 使用稳定 hash，未变化内容跳过。
- 长文档切成小 chunk，避免单条上下文过大。
- 问答只发送少量相关 chunk。
- AI 缓存 key 包含 provider、model、purpose、prompt version、content hash 和脱敏模式。
- Ingest 阶段先本地解析，再选择性 AI 增强。
- 报告生成使用分层摘要，不把整个资料集直接塞进上下文。

## 安全策略

- 默认 local-first，原始文件留在本地。
- `--dry-run-ai` 只展示将使用的 provider、模型、hash 和估算，不真正调用外部 API。
- 配置不保存真实密钥；HTTP 请求头值推荐使用 `${ENV_NAME}`。
- AI 日志默认只保存 hash、模型名、调用目的、token 估算、状态和脱敏状态。
- MCP 工具默认关闭，必须配置后才可用。
- 每个工具都应声明权限类别：`read_local`、`write_workspace`、`external_network`、`ai_external`、`mcp_external`。

## 扩展点

- AI provider：`mock` 和 `http`。新增协议应作为 adapter 接入，复用 `AiRuntime` 的安全、审计、缓存和 trace。
- MCP：连接 stdio 或 SSE MCP server，调用前执行权限检查。
- Skill：提供领域 prompt、报告章节、实体定义和问题清单，例如保险业务、供应链业务、金融风控。

## 第一版实现

- Rust CLI：`learnBusiness`。
- 工作区初始化：`.learnBusiness/config/app.toml`。
- 文档发现：文本、Markdown、基础 PDF、图片、DOCX、PPTX 元数据。
- 文本抽取：文本、Markdown、基础 PDF。
- 本地索引：SQLite + FTS5。
- 轻量切片：长文本定长切分。
- 问答：基于本地全文检索和配置指定的 AI provider；默认使用 mock。
- HTTP AI：可配置 `base_url` 和请求头，支持问答、embedding 和多模态图片请求。
- 图片入口：支持 dry-run 和 AI 调用审计记录。
- 报告：生成中文 Markdown 报告。
