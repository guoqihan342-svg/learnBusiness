## Why

learnBusiness 已经有 `AiProvider` trait、provider descriptor 和 `mock/openai-compatible/ollama/local-http` 配置入口，但非 mock provider 仍停留在安全骨架，不能真正连接本地模型或 OpenAI-compatible 服务。下一步优化需要先用 OpenSpec 固化运行时契约，确保接入本地模型时仍满足本地优先、省 token、可审计和默认安全的要求。

## What Changes

- 将 AI provider 运行时从“配置骨架”提升为可执行能力：按 `.learnBusiness/config/app.toml` 选择 provider，并执行对应的 answer、describe_image、summarize_chunks、embed_texts 调用。
- 增加 Ollama 本地模型连接器，优先支持本地 chat、vision 和 embedding 调用。
- 增加 OpenAI-compatible / local-http 的统一请求构造层，避免每个调用路径手写 HTTP。
- 在外部或本地 HTTP 调用前执行安全检查：本地 provider 只允许 localhost，外部 provider 必须从环境变量读取 API key，配置文件禁止保存密钥值。
- 在调用前应用 token 和上下文预算：只发送已检索的 top-k chunk，截断超限文本，并记录估算 token。
- 扩展 AI 调用审计：记录 provider、model、purpose、input_hash、output_hash、status、redaction_applied、token_estimate 和错误类别，不记录完整敏感 prompt。
- 更新中文操作手册、数据文档和架构文档，说明本地模型配置、dry-run、审计和排障路径。

## Capabilities

### New Capabilities

- `ai-provider-runtime`: 规定 AI provider 配置、运行时选择、本地模型连接、安全检查、token 预算、缓存和审计的用户可见行为。

### Modified Capabilities

- 无。当前仓库没有已归档 OpenSpec capability，本次先新增规格。

## Impact

- 影响代码：`src/ai/mod.rs`、`src/ai/cache.rs`、`src/ai/redaction.rs`、`src/config.rs`、`src/main.rs`、`src/qa.rs`、`src/store.rs`，可能新增 `src/ai/ollama.rs`、`src/ai/http.rs`、`src/ai/runtime.rs`。
- 影响 CLI：`ask`、`describe-image`、`inspect-ai` 需要反映配置选择、dry-run 预览、错误状态和审计记录。
- 影响配置：`.learnBusiness/config/app.toml` 的 `[ai]` 字段成为运行时行为来源，API key 只允许通过 `api_key_env` 指向环境变量。
- 影响安全：本地 provider 必须限制在 localhost；外部 provider 必须显式使用环境变量密钥；日志和审计禁止保存完整 prompt 或业务原文。
- 影响文档：README、操作手册、数据文档、架构文档、OpenSpec 设计和任务需要同步。
