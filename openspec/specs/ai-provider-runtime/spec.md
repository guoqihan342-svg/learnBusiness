## Purpose

规定 learnBusiness 的 AI provider 运行时选择、安全边界、token 预估、审计、缓存和追踪行为，确保默认 `mock` 路径和可配置 `http` 路径都通过同一套轻量、安全、可审计的入口执行。
## Requirements
### Requirement: 配置驱动的 Provider 选择
系统 SHALL 从 `.learnBusiness/config/app.toml` 的 `[ai]` 配置选择 AI provider，并在 `ask`、`describe-image`、后续摘要和 embedding 流程中使用同一套 provider 工厂。

#### Scenario: 使用 mock provider
- **WHEN** `[ai].provider` 配置为 `mock`
- **THEN** 系统 MUST 使用本地确定性 mock provider，且不执行外部网络请求。

#### Scenario: 使用通用 HTTP provider
- **WHEN** `[ai].provider` 配置为 `http`
- **THEN** 系统 MUST 使用配置中的 `base_url`、`chat_model`、`vision_model`、`embedding_model` 和 `[ai.headers]` 构造 HTTP provider 描述。

#### Scenario: 兼容旧 provider 别名
- **WHEN** `[ai].provider` 配置为 `openai-compatible`
- **THEN** 系统 MUST 将其解析为通用 HTTP provider，以兼容旧配置。

#### Scenario: 不支持的 provider
- **WHEN** `[ai].provider` 配置为未知值
- **THEN** 系统 MUST 返回明确错误，并且不得执行任何 AI 请求。

### Requirement: HTTP base_url 可配置
系统 SHALL 允许 `provider = "http"` 使用配置的 `base_url` 连接 AI 服务。`base_url` 可以是 loopback、企业网关或云端 HTTP(S) 地址；loopback 只表示本机端点，不表示必须本地部署大模型。

#### Scenario: localhost base_url 通过校验
- **WHEN** `[ai].base_url` 为 `http://localhost:8000/v1`
- **THEN** 系统 MUST 允许创建 HTTP provider descriptor，并将其标记为 loopback 端点。

#### Scenario: 远程 HTTPS base_url 通过校验
- **WHEN** `[ai].base_url` 为 `https://gateway.example.com/v1`
- **THEN** 系统 MUST 允许创建 HTTP provider descriptor，并将其视为远程端点。

#### Scenario: 非 HTTP scheme 被拒绝
- **WHEN** `[ai].base_url` 为 `file:///tmp/model`
- **THEN** 系统 MUST 拒绝配置，并提示 `base_url` 必须使用 `http` 或 `https`。

### Requirement: HTTP 请求头可配置
系统 SHALL 从 `[ai.headers]` 读取 HTTP 请求头，并在文本、embedding 和多模态请求中复用同一套请求头。请求头值 MAY 使用 `${ENV_NAME}` 环境变量占位符。

#### Scenario: 请求头环境变量被展开
- **WHEN** `[ai.headers].Authorization = "Bearer ${LEARNBUSINESS_AI_KEY}"` 且环境变量存在
- **THEN** 系统 MUST 在发送 HTTP 请求前展开该环境变量，并发送 `Authorization` 请求头。

#### Scenario: 请求头环境变量缺失时失败
- **WHEN** `[ai.headers]` 中引用的环境变量为空或不存在
- **THEN** 系统 MUST 在发起网络请求前失败，并记录 AI 调用失败审计。

#### Scenario: 多模态请求复用请求头
- **WHEN** 用户执行非 dry-run 的 `describe-image`
- **THEN** 系统 MUST 对图片理解 HTTP 请求使用与问答和 embedding 相同的 `[ai.headers]`。

#### Scenario: 请求头值不进入日志
- **WHEN** 系统写入 `ai_calls` 或 `trace.jsonl`
- **THEN** 系统 MUST NOT 保存请求头值、API key 或完整 token。

### Requirement: 密钥来源安全
系统 SHALL 禁止在配置文件中保存真实 API key 值。密钥 SHOULD 通过 `[ai.headers]` 的环境变量占位符提供。

#### Scenario: api_key_env 兼容快捷方式
- **WHEN** `[ai].api_key_env` 指向有效环境变量，且未显式配置 `Authorization` 请求头
- **THEN** 系统 MUST 在 HTTP 请求前生成 `Authorization: Bearer <环境变量值>`。

#### Scenario: api_key_env 缺失时失败
- **WHEN** `[ai].api_key_env` 指向的环境变量为空或不存在
- **THEN** 系统 MUST 在发起网络请求前失败，并提示缺少 API key/header 环境变量。

### Requirement: Dry-run 预览和审计
系统 SHALL 在 `describe-image --dry-run-ai` 中使用配置指定的 provider 和 vision model 写入 AI 调用审计，同时不得发送图片内容。

#### Scenario: HTTP dry-run
- **WHEN** provider 为 `http` 且用户执行 `describe-image --dry-run-ai`
- **THEN** `inspect-ai` 输出 MUST 包含 `provider=http` 和配置的 vision model。

#### Scenario: Dry-run 不执行网络请求
- **WHEN** 用户执行任意 dry-run AI 命令
- **THEN** 系统 MUST 只记录调用计划和输入 hash，不得执行 provider 的 HTTP 调用。

### Requirement: Token 和上下文预估
系统 SHALL 在发送 AI 请求前执行上下文预算控制，使用配置的 `performance.context_chunks` 和 `performance.chunk_char_limit` 限制发送内容。

#### Scenario: 问答使用配置的 top-k
- **WHEN** `performance.context_chunks = 2`
- **THEN** `ask` MUST 至多发送 2 个检索命中的 chunk 给 provider。

#### Scenario: 超限文本被截断或拒绝
- **WHEN** 待发送给 provider 的文本超过配置预算
- **THEN** 系统 MUST 截断到预算内或返回明确错误，并记录 token 估算状态。

### Requirement: AI 调用审计不保存敏感正文
系统 SHALL 为每次 AI 调用记录审计元数据，但不得保存完整 prompt、完整业务原文、图片原文、请求头值或 API key。

#### Scenario: 成功调用记录元数据
- **WHEN** provider 调用成功
- **THEN** 系统 MUST 记录 provider、model、purpose、input_hash、output_hash、token_estimate、redaction_applied 和 status。

#### Scenario: 调用失败记录错误状态
- **WHEN** provider 调用失败
- **THEN** 系统 MUST 记录 provider、model、purpose、input_hash、status 和错误类别，并保留本地索引结果。

### Requirement: AI 缓存按 Provider 和脱敏模式隔离
系统 SHALL 使用 provider、model、purpose、prompt_version、content_hash 和 redaction_applied 生成 AI cache key，避免不同 provider 或安全模式之间误复用结果。

#### Scenario: Provider 不同缓存不同
- **WHEN** 同一内容分别使用 `mock` 和 `http`
- **THEN** 系统 MUST 生成不同的 AI cache key。

#### Scenario: 脱敏模式不同缓存不同
- **WHEN** 同一内容分别以脱敏和未脱敏模式调用
- **THEN** 系统 MUST 生成不同的 AI cache key。

### Requirement: 本地结构化追踪日志
系统 SHALL 为 AI runtime 调用写入本地结构化追踪日志，帮助定位 provider 配置、请求状态、失败分类和耗时问题，同时不得保存完整 prompt、业务正文、图片 base64、请求头值或 API key。

#### Scenario: Provider 调用失败写入 trace
- **WHEN** provider 调用失败
- **THEN** 系统 MUST 在 `.learnBusiness/logs/trace.jsonl` 写入包含 `trace_id`、`provider`、`model`、`purpose`、`input_hash`、`status=failed`、`error_category` 和 `elapsed_ms` 的记录。

#### Scenario: 禁用追踪日志
- **WHEN** `[logging].trace_enabled = false`
- **THEN** 系统 MUST 不创建或追加 trace 日志文件，但仍保留 SQLite AI 调用审计。

### Requirement: AI 审计关联 trace
系统 SHALL 为 AI runtime 的每次调用生成 trace 标识，并在 SQLite AI 审计记录和结构化 trace 日志中保存同一个 trace id。

#### Scenario: 成功调用记录 trace id
- **WHEN** AI provider 调用成功
- **THEN** 系统 MUST 在 `ai_calls` 和 `trace.jsonl` 中记录同一个 `trace_id`。

#### Scenario: 失败调用记录 trace id
- **WHEN** AI provider 调用失败
- **THEN** 系统 MUST 在失败审计和失败 trace 事件中记录同一个 `trace_id`。

#### Scenario: Dry-run 记录 trace id
- **WHEN** 用户执行 `describe-image --dry-run-ai`
- **THEN** 系统 MUST 记录 dry-run 审计的 `trace_id`，且 MUST NOT 发送图片正文。

### Requirement: CLI 支持 AI 调用排障
系统 SHALL 通过 `inspect-ai` 输出排障所需的安全元数据，并支持按 trace id 查看相关 AI 调用。

#### Scenario: inspect-ai 输出 trace id
- **WHEN** 用户执行 `inspect-ai`
- **THEN** CLI MUST 输出每条 AI 调用的 `trace_id`、provider、model、purpose、status、error_category 和 token 估算。

#### Scenario: 按 trace id 过滤
- **WHEN** 用户执行 `inspect-ai --trace <trace_id>`
- **THEN** CLI MUST 只输出匹配该 trace id 的 AI 调用记录。

### Requirement: trace 和审计不保存敏感正文
系统 SHALL 在增强 trace 关联和排障输出后继续禁止保存完整 prompt、业务正文、图片 base64、请求头值或 API key。

#### Scenario: 失败 trace 不包含业务正文
- **WHEN** AI provider 调用失败且输入包含敏感业务文本
- **THEN** `trace.jsonl` 和 `inspect-ai` 输出 MUST NOT 包含完整输入正文或请求头值。

### Requirement: AI Runtime 关联外层操作 trace
系统 SHALL 允许 `AiRuntime` 在回答和图片理解时使用外层操作 trace id，使 AI provider 调用步骤与命令级步骤日志可以关联。

#### Scenario: ask 的 AI trace 与操作 trace 对齐
- **WHEN** `ask` 调用 `AiRuntime::answer`
- **THEN** AI 调用审计、AI trace 日志和操作步骤日志 MUST 使用同一个 trace id 或记录明确的父子关联。

#### Scenario: describe-image 的 AI trace 与操作 trace 对齐
- **WHEN** `describe-image` 或 `ingest --describe-images` 调用图片理解
- **THEN** AI 调用审计、AI trace 日志和操作步骤日志 MUST 使用同一个 trace id 或记录明确的父子关联。

### Requirement: AI Runtime 推算元数据可返回
系统 SHALL 在问答路径返回安全推算元数据，包括 trace id、检索命中数量、选中 chunk 数量、token 估算、脱敏状态和 provider 调用状态。

#### Scenario: 问答返回 token 和脱敏状态
- **WHEN** `AiRuntime::answer` 完成
- **THEN** 返回结果 MUST 包含 token 估算和是否应用脱敏的安全元数据，且 MUST NOT 包含完整 prompt 或请求头值。
