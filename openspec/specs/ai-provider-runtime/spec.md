## Purpose

规定 learnBusiness 的 AI provider 运行时选择、安全边界、token 预算、审计和缓存行为，确保本地模型、OpenAI-compatible 网关和默认 mock 路径都通过同一套轻量、安全、可审计的入口执行。

## Requirements

### Requirement: 配置驱动的 Provider 选择
系统 SHALL 从 `.learnBusiness/config/app.toml` 的 `[ai]` 配置选择 AI provider，并在 `ask`、`describe-image`、后续摘要和 embedding 流程中使用同一套 provider 工厂。

#### Scenario: 使用 mock provider
- **WHEN** `[ai].provider` 配置为 `mock`
- **THEN** 系统 MUST 使用本地确定性 mock provider，不执行外部网络请求。

#### Scenario: 使用 Ollama provider
- **WHEN** `[ai].provider` 配置为 `ollama` 且 `base_url` 是 localhost 地址
- **THEN** 系统 MUST 选择 Ollama provider，并使用配置中的 chat、vision 和 embedding 模型名构造运行时描述。

#### Scenario: 不支持的 provider
- **WHEN** `[ai].provider` 配置为未知值
- **THEN** 系统 MUST 返回明确错误，并且不得执行任何 AI 请求。

### Requirement: 本地 Provider 地址限制
系统 SHALL 将 `ollama` 和 `local-http` 视为本地 provider，并限制其 `base_url` 只能指向 `localhost`、`127.0.0.1` 或 `[::1]`。

#### Scenario: 本地地址通过校验
- **WHEN** 本地 provider 的 `base_url` 为 `http://127.0.0.1:11434`
- **THEN** 系统 MUST 允许创建 provider descriptor。

#### Scenario: 远程地址被拒绝
- **WHEN** 本地 provider 的 `base_url` 为 `https://model.example.com/v1`
- **THEN** 系统 MUST 拒绝配置并提示必须使用 localhost 地址。

### Requirement: 密钥来源安全
系统 SHALL 禁止在配置文件中保存 API key 值；外部 provider MUST 只通过 `api_key_env` 指定环境变量名来读取密钥。

#### Scenario: OpenAI-compatible 缺少密钥
- **WHEN** provider 为 `openai-compatible` 且 `api_key_env` 指向的环境变量为空或不存在
- **THEN** 系统 MUST 在发起网络请求前失败，并提示需要 API key。

#### Scenario: 本地 provider 不需要密钥
- **WHEN** provider 为 `ollama` 或 `local-http`
- **THEN** 系统 MUST 允许 `api_key_env` 为空，并不得要求用户在配置文件中写入密钥值。

### Requirement: Dry-run 预览和审计
系统 SHALL 在 `describe-image --dry-run-ai` 中使用配置指定的 provider 和 vision model 写入 AI 调用审计，同时不得发送图片内容。

#### Scenario: Ollama dry-run
- **WHEN** provider 为 `ollama` 且用户执行 `describe-image --dry-run-ai`
- **THEN** `inspect-ai` 输出 MUST 包含 `provider=ollama` 和配置的 vision model。

#### Scenario: Dry-run 不执行网络请求
- **WHEN** 用户执行任意 dry-run AI 命令
- **THEN** 系统 MUST 只记录调用计划和输入 hash，不得执行 provider 的 HTTP 调用。

### Requirement: Token 和上下文预算
系统 SHALL 在发送 AI 请求前执行上下文预算控制，使用配置的 `performance.context_chunks` 和 `performance.chunk_char_limit` 限制发送内容。

#### Scenario: 问答使用配置的 top-k
- **WHEN** `performance.context_chunks = 2`
- **THEN** `ask` MUST 至多发送 2 个检索命中的 chunk 给 provider。

#### Scenario: 超限文本被截断或拒绝
- **WHEN** 待发送给 provider 的文本超过配置预算
- **THEN** 系统 MUST 截断到预算内或返回明确错误，并记录 token 估算状态。

### Requirement: AI 调用审计不保存敏感正文
系统 SHALL 为每次 AI 调用记录审计元数据，但不得保存完整 prompt、完整业务原文、图片原文或 API key。

#### Scenario: 成功调用记录元数据
- **WHEN** provider 调用成功
- **THEN** 系统 MUST 记录 provider、model、purpose、input_hash、output_hash、token_estimate、redaction_applied 和 status。

#### Scenario: 调用失败记录错误状态
- **WHEN** provider 调用失败
- **THEN** 系统 MUST 记录 provider、model、purpose、input_hash、status 和错误类别，并保留本地索引结果。

### Requirement: AI 缓存按 Provider 和脱敏模式隔离
系统 SHALL 使用 provider、model、purpose、prompt_version、content_hash 和 redaction_applied 生成 AI cache key，避免不同 provider 或安全模式之间误复用结果。

#### Scenario: Provider 不同缓存不同
- **WHEN** 同一内容分别使用 `mock` 和 `ollama`
- **THEN** 系统 MUST 生成不同的 AI cache key。

#### Scenario: 脱敏模式不同缓存不同
- **WHEN** 同一内容分别以脱敏和未脱敏模式调用
- **THEN** 系统 MUST 生成不同的 AI cache key。

### Requirement: 本地结构化追踪日志
系统 SHALL 为 AI runtime 调用写入本地结构化追踪日志，帮助定位 provider 配置、请求状态、失败分类和耗时问题，同时不得保存完整 prompt、业务正文、图片 base64 或 API key。

#### Scenario: Provider 调用失败写入 trace
- **WHEN** provider 调用失败
- **THEN** 系统 MUST 在 `.learnBusiness/logs/trace.jsonl` 写入包含 `trace_id`、`provider`、`model`、`purpose`、`input_hash`、`status=failed`、`error_category` 和 `elapsed_ms` 的记录。

#### Scenario: 禁用追踪日志
- **WHEN** `[logging].trace_enabled = false`
- **THEN** 系统 MUST 不创建或追加 trace 日志文件，但仍保留 SQLite AI 调用审计。
