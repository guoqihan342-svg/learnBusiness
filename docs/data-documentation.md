# learnBusiness 数据文档

本文说明 learnBusiness 当前源码中的本地运行时目录、配置文件、SQLite 元数据、全文索引、AI 调用审计、缓存、产物和日志数据。learnBusiness 的默认策略是本地优先：业务文档索引、检索和审计状态写入工作区下的 `.learnBusiness/`，该目录不应提交到 Git。

## 运行时数据目录

`learnBusiness init <workspace>` 会在工作区根目录下创建 `.learnBusiness/`。当前源码中的目录布局如下：

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

`.learnBusiness/config/app.toml` 是运行时配置入口。默认内容包含 `[ai]`、`[safety]` 和 `[performance]` 三类配置：AI provider 类型、服务地址、模型名、API key 环境变量名、安全开关、问答上下文 chunk 数量、长文本切分上限。配置文件不能保存 `api_key` 值；真实密钥应通过环境变量或外部密钥管理提供。

`.learnBusiness/metadata.sqlite` 是当前已实现的核心数据文件，保存文档元数据、chunk、FTS5 全文索引和 AI 调用审计记录。`.learnBusiness/cache/ai/` 用于保存 AI 缓存结果；`.learnBusiness/artifacts/`、`.learnBusiness/cache/extraction/`、`.learnBusiness/logs/` 目前由初始化流程创建，主要作为后续图片、页面、缩略图、抽取中间结果和日志的隔离位置。

`.learnBusiness/logs/trace.jsonl` 是结构化追踪日志。每行是一条 JSON 事件，用于定位 AI 调用问题，字段包括时间、trace_id、component、operation、status、provider、model、purpose、input_hash、output_hash、token_estimate、redaction_applied、local_provider、error_category 和 elapsed_ms。它不保存 prompt、chunk 正文、图片 base64、API key 或 provider 完整返回体。

`.gitignore` 已忽略 `.learnBusiness/`。任何本地导入的业务文档内容、全文索引、AI 缓存、抽取产物和日志都应留在本机，不应提交到仓库。

## SQLite 数据库

`metadata.sqlite` 由 `MetadataStore::open` 创建，当前包含四类表：

- `documents`：每个被发现并尝试导入的文档一行。
- `chunks`：从文档文本切出的检索单元。
- `chunks_fts`：基于 FTS5 的全文检索索引。
- `ai_calls`：AI 调用审计元数据，不保存原始 prompt、原始输入或完整输出。

### documents

| 字段 | 含义 | 来源 | 生命周期 | 隐私风险 |
| --- | --- | --- | --- | --- |
| `id` | 文档稳定标识 | 导入时对文档路径字符串做 SHA-256 | 同一路径保持稳定；路径变化会生成新 id | 不含原文，但可被用于关联同一路径文档 |
| `path` | 文档文件路径 | 文档扫描结果 | 首次导入或内容变化后 upsert | 可能暴露本地目录、客户名、项目名或文件命名规则 |
| `file_type` | MIME 类型 | 文件扩展名和 MIME 猜测 | 随文档记录 upsert | 风险低，但可暴露资料类型 |
| `content_hash` | 文件内容 SHA-256 | 逐块读取文件后计算 | 内容未变时用于跳过重复导入；内容变化时更新 | 不含原文，但可用于判断两个文件内容是否相同 |
| `modified_at` | learnBusiness 记录导入的时间 | `DocumentRecord::new` 使用当前 UTC 时间 | 内容变化并重新导入时更新；当前不是文件系统 mtime | 风险低，可能暴露处理时间 |
| `size_bytes` | 文件大小 | 文件系统 metadata | 随文档记录 upsert | 风险低，可能暴露文档规模 |
| `ingest_status` | 导入状态 | 当前成功路径写入 `indexed` | upsert 时覆盖；当前跳过和告警主要体现在命令摘要中 | 风险低，可暴露处理结果 |

重复导入时，learnBusiness 会先读取 `documents.content_hash`。如果数据库中的 hash 与当前文件 hash 相同，导入流程直接跳过该文档，不重新抽取文本，也不重建 chunk。

### chunks

| 字段 | 含义 | 来源 | 生命周期 | 隐私风险 |
| --- | --- | --- | --- | --- |
| `id` | chunk 稳定标识 | UUID v5，种子包含 `document_id`、类型、序号位置和 chunk 内容 hash | 同一文档、同一位置、同一内容保持稳定；内容变化会生成新 id | 不含原文，但可关联 chunk 是否变化 |
| `document_id` | 所属文档 id | 导入流程传入 | 文档内容变化时，旧 chunk 先清理再写入新 chunk | 可关联到 `documents` |
| `kind` | chunk 类型 | 当前文本导入写入 `text` | 当前实现只持久化文本 chunk；其他类型为模型结构预留 | 风险低 |
| `text` | chunk 原文 | 文本、Markdown 或 PDF 抽取结果切分后写入 | 文档变化时删除旧 chunk 并写入新 chunk | 高风险，可能包含完整业务规则、客户信息、合同条款或内部流程 |
| `page` | 当前 chunk 序号 | 导入流程传入 `1..n` | 重新切分时随新 chunk 写入；当前不是 PDF 原始页码 | 风险低，可暴露文档结构 |
| `slide` | 幻灯片序号 | 当前文本导入传入空值 | 预留给 PPT 或多模态抽取 | 风险低 |
| `source_range` | 原文范围 | 当前 `insert_chunk` 未写入 | 预留字段 | 若未来写入，可能暴露页码、段落或坐标 |
| `artifact_path` | 产物路径 | 当前 `insert_chunk` 未写入 | 预留字段 | 若未来写入，可能暴露本地文件路径 |
| `confidence` | 抽取置信度 | 当前 `insert_chunk` 未写入 | 预留字段 | 风险低 |
| `ai_generated` | 是否为 AI 生成内容 | 当前默认 `0` | 预留给 AI 摘要、OCR 等 chunk | 风险低，但可暴露内容来源 |
| `content_hash` | chunk 文本 SHA-256 | `insert_chunk` 对 `text` 计算 | chunk 更新时同步更新 | 不含原文，但可用于判断内容是否相同 |
| `created_at` | 数据库创建时间 | SQLite 默认时间 | 首次插入时生成；冲突更新不重置 | 风险低，可能暴露导入时间 |

长文本会按默认 `chunk_char_limit = 1600` 字符切分。当前切分按字符数顺序累积，不做语义段落合并；每个 chunk 写入 `chunks.text`，并同步写入 `chunks_fts`。这样可以降低单次问答发送给 AI 的上下文大小，也避免把整篇文档作为一个检索单元。

当文档 hash 变化时，learnBusiness 先调用 `delete_chunks_for_document` 删除该文档在 `chunks_fts` 和 `chunks` 中的旧数据，再写入新 chunk。这样可以避免旧业务规则继续被全文检索或问答引用。

### chunks_fts

| 字段 | 含义 | 来源 | 生命周期 | 隐私风险 |
| --- | --- | --- | --- | --- |
| `chunk_id` | FTS 记录关联的 chunk id | `insert_chunk` 写入 | chunk 更新前先删除旧 FTS 记录，再插入新记录 | 可关联到 `chunks` |
| `document_id` | FTS 记录关联的文档 id | `insert_chunk` 写入 | 文档变化时随旧 chunk 清理 | 可关联到 `documents` |
| `text` | FTS5 索引文本 | chunk 文本加分词辅助内容 | 与 chunk 同步写入和清理 | 高风险，重复保存可检索的业务文本 |

FTS5 表用于 `ask` 和报告生成中的全文搜索。写入索引时，learnBusiness 会保留原始 chunk 文本，并补充空白分词片段以及 2 字、3 字窗口，提升中文短词检索命中率。查询时会把问题整理为短语和窗口词，并按 BM25 分数排序。

`chunks_fts` 是业务文本的另一份可检索副本，不能提交、同步到公开存储或作为普通日志导出。

### ai_calls

| 字段 | 含义 | 来源 | 生命周期 | 隐私风险 |
| --- | --- | --- | --- | --- |
| `id` | AI 调用记录 id | 对 provider、model、purpose、input_hash、status 计算 SHA-256 | 相同审计维度会更新同一记录 | 不含原始输入，但可关联同一调用 |
| `task_id` | 任务类型 | 当前按 `purpose` 写入，例如 `answer` 或 `describe_image` | 插入时写入 | 风险低 |
| `provider` | AI 服务提供方 | 调用方传入 | 插入或冲突更新时保留 | 可暴露供应商选择 |
| `model` | 模型名 | 调用方或 mock provider 返回 | 插入或冲突更新时保留 | 可暴露模型配置 |
| `purpose` | 调用目的 | runtime 写入 `answer`、`describe_image` 等 | 插入或冲突更新时保留 | 风险低 |
| `input_hash` | 输入内容 hash | 图片等输入内容 SHA-256 | 插入或冲突更新时保留 | 不含原图，但可判断输入是否相同 |
| `output_hash` | 输出内容 hash | provider 成功返回后对输出文本计算；dry-run 和失败为空 | 冲突更新时可更新 | 不写入原始输出 |
| `token_estimate` | token 估算 | runtime 在调用前按轻量字符估算写入 | 冲突更新时可更新 | 风险低 |
| `redaction_applied` | 是否已脱敏 | 外部 provider 且开启脱敏时写入 `true` | 冲突更新时可更新 | 可暴露安全处理状态 |
| `error_category` | 失败类别 | provider 或配置失败时写入，例如 `api_key_missing`、`http_request`、`invalid_response`、`provider_failed` | 冲突更新时可更新 | 不含错误正文，但可暴露失败类型 |
| `status` | 调用状态 | 当前写入 `dry_run`、`completed` 或 `failed` | 冲突更新时可更新 | 风险低 |
| `created_at` | 数据库创建时间 | SQLite 默认时间 | 首次插入时生成；冲突更新不重置 | 风险低 |

AI 调用记录的设计目标是审计，而不是日志正文。`inspect-ai` 输出的是 purpose、provider、model、status、input_hash、output_hash、redaction、token 估算和 error_category 等元数据。AI 日志只应保留 hash、状态、模型、失败分类和脱敏标记这类审计信息，不应写入原始业务文档、图片内容、prompt、回答正文、provider 返回体或 API key。

## AI cache

AI cache 位于 `.learnBusiness/cache/ai/`。缓存文件名由 `AiCacheKey` 生成，参与 hash 的字段包括 provider、model、purpose、prompt_version、content_hash 和 redaction_applied，最终文件名形如 `<sha256>.json`。cache key 不包含 prompt、上下文正文、图片 base64、模型回答正文或 API key。

当前非 dry-run 的图片理解流程会把模型返回的描述写入 AI cache。缓存文件名本身只暴露 hash，但缓存内容可能包含对业务图片、流程图、截图或文档页面的文字描述，因此也属于敏感运行时数据。缓存只能作为本地复用数据，不应提交、共享或放入普通日志。

## artifacts、extraction cache 和 logs

`.learnBusiness/artifacts/images/`、`.learnBusiness/artifacts/pages/`、`.learnBusiness/artifacts/thumbnails/` 用于隔离图片、页面渲染和缩略图等产物。当前源码会创建这些目录，并在抽取模型中预留 `artifact_path` 字段；文本导入路径尚未把 artifact 写入 `chunks`。

`.learnBusiness/cache/extraction/` 用于后续抽取中间结果缓存。当前源码只创建目录，尚未实现持久化写入。

`.learnBusiness/logs/` 用于后续运行日志。安全要求是日志只记录必要状态、hash、耗时、错误分类等审计信息，不记录业务原文、AI prompt、AI 完整回答、provider 完整返回体、请求体、图片内容、API key 或外部服务 token。

`trace.jsonl` 可通过 `[logging].trace_enabled = false` 关闭。关闭后不会创建或追加追踪日志，但 `ai_calls` 审计仍会写入 SQLite。

## hash、稳定 id 和去重

learnBusiness 当前使用三类稳定标识：

- 文档内容 hash：对文件字节流计算 SHA-256，保存在 `documents.content_hash`，用于判断重复导入和文件是否变化。
- 文档 id：对文档路径字符串计算 SHA-256，保存在 `documents.id`，用于连接文档与 chunk。
- chunk id：用 UUID v5 生成，种子包含文档 id、chunk 类型、位置序号和 chunk 内容 hash。只要这些输入不变，chunk id 保持稳定。

重复导入时，如果文档内容 hash 未变化，learnBusiness 会跳过该文档，避免重复抽取、重复写入和重复扩大索引。文档变化时，旧 chunk 和旧 FTS 记录会先被删除，再写入新 chunk，避免问答引用过期内容。

## 问答来源引用

`ask` 会先读取 `.learnBusiness/config/app.toml` 中的 `performance.context_chunks`，再在 `chunks_fts` 中搜索问题相关内容。默认值是 `DEFAULT_CONTEXT_CHUNKS = 5`，用户可在配置文件中调小或调大；当前实现会把有效值限制在安全范围内，避免一次发送过多上下文。没有命中时，问答流程直接返回无来源结果，并且不会调用 AI。

有命中时，learnBusiness 只把命中的 chunk id 和 chunk 文本组成上下文传给 AI provider。回答结果返回后，来源引用来自命中结果中的 `documents.path`，会去重后输出为“来源”。当前用户可见来源是文档路径，不是页码或段落范围；`source_range`、`page` 和 `slide` 等更细粒度引用字段仍是预留方向。

这个流程的安全边界是：AI 只看到检索命中的少量 chunk，而不是整个文档库；但命中 chunk 仍可能包含敏感业务文本，因此需要配合脱敏、安全配置和本地审计策略使用。

## 安全要求

- 不提交 `.learnBusiness/`，包括 `metadata.sqlite`、FTS5 索引、AI cache、artifacts、logs 和 extraction cache。
- `.learnBusiness/config/app.toml` 不保存 `api_key`、访问令牌或个人账号凭据。
- 本地模型 provider 使用 `ollama` 或 `local-http` 时，`base_url` 必须是 localhost 地址，避免把本地资料误发到远程服务。
- AI 调用审计只保存 hash、模型、用途、状态、token 估算、失败分类和脱敏标记等元数据。
- 普通日志不记录业务原文、AI prompt、AI 完整回答、图片内容或密钥。
- `chunks.text` 和 `chunks_fts.text` 是最高敏数据，应按业务文档原文处理。
- AI cache 可能包含模型输出的业务摘要或图片描述，应按敏感数据处理。
- 对外发送 AI 请求前应启用脱敏策略；当前配置默认包含 `redact_before_external_ai`，但实际接入外部 provider 时仍需要逐调用确认。
