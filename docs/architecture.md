# learnBusiness 架构文档

## 项目定位

learnBusiness 是一个本地优先的业务文档理解工具，用于把散落在本机目录里的业务资料转成可检索、可问答、可审计的工作区索引。它优先处理 PDF、文本、Markdown、图片、Word、PPT 等业务资料，而不是假设输入一定是代码仓库。

核心设计目标是本地优先、轻量、省 token、安全优先：

- 本地优先：默认在本机完成发现、抽取、切块、索引、检索和报告生成，索引、缓存、配置和调用记录都落在工作区的 `.learnBusiness/` 下。
- 轻量：用 Rust CLI、SQLite、FTS 和文件缓存承载核心能力，避免一开始引入重型服务端、队列或向量数据库。
- 省 token：先用本地全文检索缩小上下文，只把少量命中的 chunk 交给问答或摘要流程；默认上下文数量为 5，默认 chunk 大小为 1600 字符。
- 安全优先：默认配置不写入 API key，外部 AI 前预留脱敏链路，AI 调用记录只保存哈希、状态、token 估计和是否脱敏等审计字段。

## 工作区结构

`workspace` 模块把用户指定的业务目录视为根目录，并在根目录下维护 `.learnBusiness/`。初始化后主要结构包括：

- `.learnBusiness/config/app.toml`：集中配置 AI、安全和性能参数。
- `.learnBusiness/metadata.sqlite`：保存文档元数据、chunk、全文索引和 AI 调用记录。
- `.learnBusiness/cache/ai/`：保存可复用的 AI 输出缓存。
- `.learnBusiness/logs/trace.jsonl`：保存 AI runtime 结构化追踪事件。
- `.learnBusiness/fulltext/`、`.learnBusiness/vectors/`、`.learnBusiness/artifacts/images/`：为全文、向量和图片类中间产物预留落点。

工作区目录是 learnBusiness 的主要写入边界。除用户显式指定的报告输出路径外，运行时状态应尽量留在 `.learnBusiness/` 内。

## 模块职责

### main

`main` 是 CLI 编排层，负责解析命令并把请求转交给具体模块。当前命令包括 `init`、`ingest`、`status`、`inspect-ai`、`report`、`ask` 和 `describe-image`。它只做流程串联和输出命令结果；AI 调用、dry-run、审计和缓存由 `AiRuntime` 统一处理。

### config

`config` 定义项目名、工作区目录名、默认性能参数和 `AppConfig`。配置文件固定为 `.learnBusiness/config/app.toml`，包含：

- `ai`：provider、base_url、chat_model、vision_model、embedding_model、api_key_env。
- `safety`：redact_before_external_ai、dry_run_ai。
- `performance`：context_chunks、chunk_char_limit。
- `logging`：trace_enabled。

默认 provider 是 `mock`，默认不把 API key 写进配置文件，只保存 API key 所在环境变量名。当前 provider registry 支持 `mock`、`openai-compatible`、`ollama` 和 `local-http`。

### workspace

`workspace` 管理工作区布局。`init` 会创建 `.learnBusiness/`、配置目录、全文目录、向量目录、图片产物目录和 AI 缓存目录，并在缺少配置时写入默认配置。`open` 用于在已有工作区上定位 metadata、cache 和配置路径。

### discover

`discover` 遍历业务文档目录，过滤支持的文件类型，识别 MIME 风格的文件类型，计算文件 SHA-256，并记录文件大小。它是 ingest 的入口筛选层，负责在进入抽取前建立稳定的文件身份和变更判断基础。

### ingest/extract

`ingest` 串联文档发现、内容抽取、增量判断、切块和入库。它用文档路径生成稳定文档 ID，用文件 hash 判断未变更文档并跳过重复处理。文本会按默认 1600 字符切成 chunk，再写入 `store`。

`extract` 负责把不同文件类型转换成可索引文本。当前文本、Markdown 和 PDF 可以直接抽取；图片、Word、PPT 会返回 artifact_path 并标记 needs_ai，表示需要后续 AI/OCR 或多模态流程补齐内容。

### store

`store` 是持久化层，基于 SQLite 管理：

- `documents`：文档路径、类型、内容 hash、大小、入库状态。
- `chunks`：chunk 文本、页码、幻灯片、内容 hash 和 AI 生成标记。
- `chunks_fts`：SQLite FTS5 全文索引。
- `ai_calls`：AI 调用审计记录。

它提供 upsert 文档、替换 chunk、全文检索、列出 chunk、统计文档数量、记录和查看 AI 调用等能力。

### ai/runtime/cache/redaction

`ai` 定义 `AiProvider` 抽象，覆盖图片理解、chunk 摘要、embedding 和问答。当前有确定性的 `MockAiProvider`，以及可执行 HTTP 调用的 `OpenAiCompatibleProvider`、`OllamaProvider` 和 `LocalHttpProvider`。`AiProviderDescriptor` 负责把配置解析成 provider 类型、模型名、是否需要 API key、是否只允许本地地址、是否支持视觉和 embedding。

`AiRuntime` 是统一 AI 网关，持有 `AppConfig`、provider descriptor、workspace 路径、provider 实例和 trace logger。`ask` 和 `describe-image` 都通过它执行 provider 校验、top-k 上下文控制、chunk 长度截断、外部 provider 脱敏、轻量 token 估算、成功/失败审计、AI cache 写入和结构化追踪。

provider 模块已拆分为：

- `runtime.rs`：统一调用网关和 token 估算。
- `http.rs`：共享 blocking HTTP JSON 调用和 URL helper。
- `ollama.rs`：Ollama `/api/chat`、vision 和 `/api/embeddings`。
- `openai.rs`：OpenAI-compatible chat、vision 和 embeddings。
- `local_http.rs`：本机轻量 JSON 网关协议。
- `mod.rs`：公共 trait、类型、descriptor 和导出入口。

`cache` 用 provider、model、purpose、prompt_version、content_hash 和 redaction_applied 生成稳定缓存文件名，避免重复消耗 token。

`redaction` 提供外部 AI 前的文本脱敏工具，当前覆盖 API key、邮箱、手机号和长数字。

### qa

`qa` 负责检索增强问答。它会读取 `.learnBusiness/config/app.toml` 中的 `performance.context_chunks`，再用 `store.search_text` 按问题检索对应数量的相关 chunk；没有命中时直接返回“未找到相关来源”，不调用 AI；有命中时把 chunk 作为上下文交给 provider，并返回去重后的来源路径。

### report

`report` 从本地索引生成 Markdown 报告。它读取文档数量和前若干 chunk，生成执行摘要、可能的流程或规则线索、来源引用等内容。当前报告偏轻量，主要用于快速审阅索引覆盖和业务线索。

### task

`task` 定义面向 agent 和工具扩展的描述结构，包括 `Permission`、`PermissionSet`、`ToolDescriptor`、`AgentDescriptor` 和 `TaskDescriptor`。它把读本地、写工作区、外部网络、外部 AI、外部 MCP 等权限显式建模，为后续 tool loop、MCP、skill 和多 agent 编排提供安全边界。

## 数据流

### init

用户执行 init 后，`main` 调用 `Workspace::init`。工作区会创建 `.learnBusiness/` 布局，写入默认 `app.toml`，并保证重复 init 不破坏已有配置。

数据流：

`main` -> `workspace` -> `.learnBusiness/config/app.toml` 和工作区子目录

### ingest

用户执行 ingest 后，`main` 调用 `run_ingest`。流程先打开工作区和 SQLite，再由 `discover` 扫描文档目录。每个文档会先比较已有 content_hash，未变化则跳过；变化或首次出现时进入 `extract`，可直接抽取的文本会被切块并写入 `store`，旧 chunk 会先删除再替换。

数据流：

`main` -> `ingest` -> `discover` -> `extract` -> `store.documents` / `store.chunks` / `store.chunks_fts`

### ask

用户执行 ask 后，`qa` 调用 `AiRuntime::answer`。runtime 打开 metadata，先用全文索引检索与问题相关的 chunk，再按 `performance.context_chunks` 和 `performance.chunk_char_limit` 控制上下文。命中后构造少量上下文交给 provider，并记录成功或失败审计；没有命中时不调用 AI。

数据流：

`main` -> `qa` -> `AiRuntime::answer` -> `store.search_text` -> `AiProvider.answer` -> `store.ai_calls` -> answer + sources

### report

用户执行 report 后，`report` 读取本地索引中的文档数量和 chunk 摘要，生成 Markdown 报告并写到用户指定路径。

数据流：

`main` -> `report` -> `store.document_count` / `store.list_chunks` -> 报告文件

### describe-image dry-run/调用

`describe-image` 支持 dry-run 和实际调用两种路径。

dry-run 路径会通过 `AiRuntime::describe_image` 计算图片 hash 和文件类型，写入一条 `status=dry_run` 的 AI 调用记录，并输出 provider、model、purpose、图片路径、input_hash、MIME、脱敏状态、token 估计和是否本地 provider；它不调用 AI，也不写 AI 缓存。

实际调用路径同样经过 `AiRuntime::describe_image`，构造 `ImageInput`，调用当前 provider 生成图片描述，写入 `status=completed` 的 AI 调用记录，并用 `AiCacheKey` 把结果保存到 `.learnBusiness/cache/ai/`。如果 provider 失败，runtime 写入 `status=failed` 和 `error_category`，但不保存完整 prompt、图片 base64 或 provider 返回体。

数据流：

`main` -> `AiRuntime::describe_image` -> `discover.sha256_file` / `discover.guess_file_type` -> `store.ai_calls` -> `AiProvider.describe_image` -> `ai/cache`

### inspect-ai

用户执行 inspect-ai 后，`main` 打开 metadata 并列出所有 AI 调用记录。输出字段包括 purpose、provider、model、status、input_hash、output_hash、redaction、token_estimate 和 error_category，用于审计是否发生外部 AI、dry-run 或失败调用。

数据流：

`main` -> `store.list_ai_calls` -> 审计输出

## 安全边界

learnBusiness 的安全边界按“默认本地、外部显式、可审计”设计：

- 默认数据处理在本机完成，核心写入位置是 `.learnBusiness/`。
- API key 不写入默认配置，应通过运行时注入或环境变量等外部秘密管理方式提供。
- AI 调用记录保存输入 hash、输出 hash、provider、model、purpose、状态、token 估计、失败分类和是否脱敏，避免直接把敏感原文写进调用日志。
- 外部 AI 前由 `AiRuntime` 调用 `redaction` 脱敏工具；本地 provider 不强制脱敏，但仍只保存审计元数据。
- dry-run 用于在真实调用前检查会发生什么，适合调试权限、成本和审计链路。
- `ollama` 和 `local-http` 强制 `base_url` 指向 loopback 地址；远程本地 provider URL 会在 runtime 创建阶段被拒绝。
- 图片 base64 只进入 provider 请求体，不进入 `ai_calls` 审计记录或错误分类。
- provider 错误信息只分类写入审计；日志和审计不保存完整上下文、请求体或 provider 完整返回体。
- `trace.jsonl` 只记录定位问题所需的元数据、hash、耗时和错误分类，不记录业务正文。

## 权限模型

`task` 模块把权限拆成五类：

- `ReadLocal`：读取本地业务文档或工作区状态。
- `WriteWorkspace`：写入 `.learnBusiness/`、缓存、索引或产物。
- `ExternalNetwork`：访问外部网络。
- `AiExternal`：调用外部 AI。
- `McpExternal`：调用外部 MCP 工具。

`ToolDescriptor::ensure_allowed` 会用 `PermissionSet` 校验工具是否被授权。当前 CLI 主流程还没有把这个权限模型贯穿到每个命令，但模型已经为后续 agent 工具调用、MCP 适配器和更细粒度授权预留了接口。

## AI provider、MCP 和 skill 扩展点

AI provider 的核心扩展点是 `AiProvider` trait。新的 provider 只要实现图片理解、摘要、embedding 和问答接口，就可以接入现有 `AiRuntime`、qa、describe-image 和未来 ingest 补全流程。OpenAI-compatible provider 使用 base_url、chat_model、vision_model、embedding_model 和 api_key_env 配置字段；Ollama 和 Local HTTP provider 面向本机模型服务，默认不需要 API key，并强制 localhost 地址。

MCP 扩展点应落在 `task` 权限模型之后：MCP 工具需要声明 `McpExternal` 或更细的派生权限，并通过 `ToolDescriptor` 校验后再执行。这样可以避免业务文档 agent 在无授权情况下访问外部系统。

skill 扩展点适合承载领域流程和提示模板，例如“合同审查”“采购流程梳理”“系统操作手册问答”等。skill 不应绕过检索、脱敏、缓存和审计链路，而应作为任务描述、prompt_version 或 agent 流程的一部分接入。

## 性能策略

当前性能策略强调少做、复用和先过滤：

- 增量 ingest：用 content_hash 跳过未变更文档。
- 稳定 ID：文档和 chunk 使用稳定 hash 或 UUID 生成，便于替换和缓存。
- 有界切块：默认 chunk_char_limit 为 1600，避免单次上下文过大。
- 有界问答上下文：默认 `context_chunks` 为 5，可在工作区配置里调整，并限制最大值以控制 token 成本。
- 本地全文索引：SQLite FTS5 支撑轻量检索，不依赖外部检索服务。
- AI 缓存：缓存 key 包含模型、用途、prompt 版本、内容 hash 和脱敏状态，避免复用错误结果。
- 空命中短路：问答无来源时不调用 AI，直接返回无来源结果。
- dry-run：在真实 AI 调用前验证成本和审计路径。

## 当前限制

learnBusiness 当前仍是轻量原型，主要限制包括：

- 真实 provider 调用依赖本机或网关服务可用；Ollama 需要提前启动服务并拉取模型。
- `local-http` 只支持 learnBusiness 定义的最小 JSON 协议；其他模型服务需要适配到 `/answer`、`/describe-image`、`/embeddings`。
- OpenAI-compatible 网关必须提供标准 chat completions、vision payload 和 embeddings 兼容响应；非标准扩展需要后续 provider 增量支持。
- ingest 当前只直接抽取文本、Markdown 和 PDF；图片、Word、PPT 只标记为需要 AI，尚未自动 OCR 或多模态补全。
- 向量目录已经预留，但当前检索主要依赖 SQLite FTS5，embedding 和向量搜索尚未成为主流程。
- report 是规则和样例驱动的轻量报告，不等同于完整业务建模。
- redaction 目前是正则级脱敏，只覆盖常见邮箱、手机号、长数字和 API key。
- 权限模型已经定义，但还没有在每个 CLI 命令上形成统一执行网关。
- 并发 ingest、失败重试、任务队列、细粒度审计导出和复杂 Office 结构抽取仍待完善。
