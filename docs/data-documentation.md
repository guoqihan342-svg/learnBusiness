# learnBusiness 数据文档

本文说明 learnBusiness 当前源码中的运行时目录、配置文件、SQLite 元数据、全文索引、AI 调用审计、缓存、artifact 和日志数据。默认策略是本地优先：业务文档索引、检索和审计状态写入工作区下的 `.learnBusiness/`，该目录不应提交到 Git。

## 运行时目录

`learnBusiness init <workspace>` 会在工作区根目录下创建：

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
    trace.jsonl
    operations.jsonl
```

`.learnBusiness/config/app.toml` 是运行时配置入口。配置包含 AI provider、HTTP `base_url`、模型名、请求头、脱敏开关、问答上下文数量、chunk 长度上限、AI trace 和 operation trace 开关。

配置文件不应保存真实密钥值。推荐在 `[ai.headers]` 中保存环境变量占位符，例如：

```toml
[ai.headers]
Authorization = "Bearer ${LEARNBUSINESS_AI_KEY}"
X-App = "learnBusiness"
```

## SQLite 数据库

`metadata.sqlite` 由 `MetadataStore::open` 创建，当前包含四类表。

### documents

| 字段 | 含义 | 来源 | 隐私风险 |
| --- | --- | --- | --- |
| `id` | 文档稳定标识 | 文档路径 hash | 不含原文，但可关联同一路径 |
| `path` | 文档路径 | 扫描结果 | 可能暴露客户名、项目名或本地目录 |
| `file_type` | MIME 类型 | 扩展名和 MIME 猜测 | 风险较低 |
| `content_hash` | 文件内容 SHA-256 | 读取文件字节 | 不含原文，可判断内容是否相同 |
| `modified_at` | 入库时间 | 当前 UTC 时间 | 风险较低 |
| `size_bytes` | 文件大小 | 文件系统 metadata | 风险较低 |
| `ingest_status` | 导入状态 | ingest 流程 | 风险较低 |

重复导入时会先比较 `content_hash`。未变化的文档会跳过，避免重复抽取、写入和后续 AI 调用。

### chunks

| 字段 | 含义 | 来源 | 隐私风险 |
| --- | --- | --- | --- |
| `id` | chunk 稳定标识 | UUID v5 | 不含原文 |
| `document_id` | 所属文档 id | ingest 流程 | 可关联到 documents |
| `kind` | chunk 类型 | 当前主要为 `text` | 风险较低 |
| `text` | chunk 原文 | 文本/PDF 抽取后切分 | 高风险，可能包含完整业务规则或敏感信息 |
| `page` | 页码或可用位置 | 抽取器/ingest 流程 | 风险较低 |
| `slide` | 幻灯片序号 | PPTX 抽取器 | 风险较低 |
| `source_range` | 原文范围 | 抽取器/ingest 流程 | 可能暴露页码/段落 |
| `artifact_path` | artifact 路径 | 抽取器/ingest 流程 | 可能暴露本地路径 |
| `confidence` | 抽取置信度 | 预留字段 | 风险较低 |
| `ai_generated` | 是否 AI 生成 | 当前默认为 false | 可暴露内容来源 |
| `content_hash` | chunk 文本 hash | `text` hash | 不含原文 |
| `created_at` | 创建时间 | SQLite 默认时间 | 风险较低 |

长文本按 `chunk_char_limit` 切分，默认 1600 字符。文档内容变化时，旧 chunk 和旧 FTS 记录会先删除，再写入新 chunk。`.docx` 正文会以文本 chunk 入库；`.pptx` 幻灯片文本会写入文本 chunk，并记录 `slide` 编号。显式执行 `ingest --describe-images` 时，图片描述会以 `kind = image`、`ai_generated = true` 的 chunk 写入，并通过 `artifact_path` 指回原图片。

### chunks_fts

`chunks_fts` 是 SQLite FTS5 全文索引，保存可检索文本副本。它用于 `ask` 和报告生成。该表同样包含敏感业务文本，不应导出、提交或复制到公开位置。

### ai_calls

| 字段 | 含义 | 来源 | 隐私边界 |
| --- | --- | --- | --- |
| `id` | 调用记录 id | provider/model/purpose/input_hash/status hash | 不含原始输入 |
| `task_id` | 任务类型 | 当前等同 purpose | 风险较低 |
| `provider` | provider 名称 | `AiRuntime` | 暴露使用 `mock` 或 `http` |
| `model` | 模型名 | 配置或 provider 返回 | 暴露模型配置 |
| `purpose` | 调用目的 | `answer`、`describe_image` 等 | 风险较低 |
| `input_hash` | 输入 hash | 问题+上下文或图片内容 hash | 不含原文 |
| `output_hash` | 输出 hash | 成功返回后计算 | 不保存输出正文 |
| `trace_id` | trace 关联标识 | `AiRuntime` | 不含原文，可定位 trace 日志 |
| `token_estimate` | token 估算 | runtime 轻量估算 | 风险较低 |
| `redaction_applied` | 是否脱敏 | runtime 判断 | 暴露安全处理状态 |
| `error_category` | 失败类别 | runtime 分类 | 不保存完整错误正文 |
| `status` | 调用状态 | `dry_run`、`completed`、`failed` | 风险较低 |
| `created_at` | 创建时间 | SQLite 默认时间 | 风险较低 |

`ai_calls` 的目标是审计，不是日志正文。它不保存 prompt、上下文原文、图片 base64、请求头值、provider 完整返回体或真实 API key。`trace_id` 用于把审计记录和 `.learnBusiness/logs/trace.jsonl` 中的结构化事件关联起来。

## Trace 日志

`.learnBusiness/logs/trace.jsonl` 每行是一条 JSON 事件，用于定位 AI 调用问题。字段包括：

- `trace_id`
- `timestamp`
- `component`
- `operation`
- `status`
- `provider`
- `model`
- `purpose`
- `input_hash`
- `output_hash`
- `token_estimate`
- `redaction_applied`
- `local_provider`
- `error_category`
- `elapsed_ms`

当前字段名 `local_provider` 表示 `base_url` 是否为 loopback HTTP 端点，不表示一定部署了本地大模型。trace 不记录请求头值、prompt、chunk 正文、图片 base64 或完整响应。

可通过配置关闭 trace：

```toml
[logging]
trace_enabled = false
```

关闭后不会创建或追加 `trace.jsonl` 和 `operations.jsonl`，但 SQLite `ai_calls` 审计仍会写入。

## AI Cache

AI cache 位于 `.learnBusiness/cache/ai/`。缓存文件名由 `AiCacheKey` 生成，参与 hash 的字段包括：

- provider
- model
- purpose
- prompt_version
- content_hash
- redaction_applied

cache key 不包含 prompt、上下文正文、图片 base64、请求头值、模型回复正文或 API key。

当前非 dry-run 的图片理解流程会把模型返回的描述写入 AI cache。缓存内容可能包含对业务图片、流程图、截图或文档页面的文字描述，应按敏感运行时数据处理。

图片描述入库会同时写入 `chunks.text` 和 AI cache。`--dry-run-ai` 只写 AI 调用审计和 trace，不写图片描述 chunk。

## 配置数据

默认配置示例：

```toml
[ai]
provider = "mock"
base_url = "http://localhost:8000/v1"
chat_model = "business-chat"
vision_model = "business-vision"
embedding_model = "business-embedding"
api_key_env = ""

[ai.headers]
# Authorization = "Bearer ${LEARNBUSINESS_AI_KEY}"

[safety]
redact_before_external_ai = true
dry_run_ai = false

[performance]
context_chunks = 5
chunk_char_limit = 1600

[logging]
trace_enabled = true
```

## Operation Trace 数据

`.learnBusiness/logs/operations.jsonl` 保存命令级步骤日志，每行一条 JSON 事件。它用于定位 `ingest`、`search`、`ask` 和 `describe-image` 的运行路径，和 AI 调用日志 `.learnBusiness/logs/trace.jsonl` 互补。

主要字段包括：

- `trace_id`：一次操作的关联标识。
- `operation`：命令或操作名，例如 `ingest`、`search`、`ask`。
- `component`：组件名，例如 `store`、`AiRuntime`。
- `step`：步骤名，例如 `local_search`、`write_index`。
- `status`：`started`、`completed`、`failed`、`skipped`、`dry_run` 等状态。
- `input_hash` / `output_hash`：输入和输出 hash，不保存原文。
- `result_count`：命中或写入数量。
- `token_estimate`：token 估算值。
- `redaction_applied`：是否脱敏。
- `error_category`：错误分类。
- `elapsed_ms`：步骤耗时。
- `message`：只允许写入数量、limit、状态等安全短摘要。

禁止写入的数据包括完整 prompt、完整业务正文、chunk 正文、图片 base64、HTTP 请求头真实值、API key、token、provider 完整请求体或完整响应体。

`provider = "http"` 后，问答、embedding 和多模态图片请求都使用同一个 `base_url` 和同一套 `[ai.headers]`。`api_key_env` 仅作为兼容快捷方式，不建议再新增只依赖它的配置。

## 安全要求

- 不提交 `.learnBusiness/`。
- 不在 `app.toml` 中保存真实 token、API key 或个人账号凭据。
- `[ai.headers]` 可以保存 header 名和环境变量占位符，但不应保存真实 header 值。
- 普通日志和 trace 不记录业务原文、AI prompt、完整回答、图片内容、请求体、请求头值或 provider 完整返回体。
- `chunks.text` 和 `chunks_fts.text` 是最高敏感数据，应按业务文档原文处理。
- AI cache 可能包含模型输出的业务摘要或图片描述，也应按敏感数据处理。
- 接入远程 HTTP AI 前应保持 `redact_before_external_ai = true`。
