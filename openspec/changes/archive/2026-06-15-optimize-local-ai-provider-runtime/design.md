## Context

learnBusiness 当前已经完成本地索引、可配置 top-k、AI provider descriptor 和 `mock/openai-compatible/ollama/local-http` provider 骨架。现状的主要问题是：非 mock provider 只返回“HTTP 执行未实现”，本地模型还不能真正用于问答、图片理解或 embedding；同时 token 预算、脱敏、审计和 cache key 虽然已有部分模块，但还没有在真实 provider 调用路径上形成统一网关。

该优化面向下一轮实现，不改变 local-first 的默认策略。默认配置仍使用 `mock`，只有用户显式配置 `ollama`、`local-http` 或 `openai-compatible` 时才进入对应 provider。所有文档和配置继续使用中文说明，并保持 `.learnBusiness/config/app.toml` 作为唯一运行时配置入口。

## Goals / Non-Goals

**Goals:**

- 让 `ask`、`describe-image`、摘要和 embedding 能通过同一 provider runtime 调用不同 AI provider。
- 优先支持本地 Ollama：chat、vision、embedding 请求结构可测试，错误清晰。
- 为 OpenAI-compatible 和通用 local-http 保留统一 HTTP 请求构造和响应解析边界。
- 在 provider 调用前统一执行本地地址校验、API key 环境变量读取、脱敏、token 预算和 dry-run。
- 在 provider 调用后统一写入审计和 cache，确保不保存敏感正文。
- 保持轻量：不引入重型运行时、服务端队列或外部数据库。

**Non-Goals:**

- 不在本 change 中实现完整多 agent 自主规划。
- 不强制用户安装 Ollama 或任何本地模型。
- 不把 `.learnBusiness/` 中的索引、缓存或审计同步到远端。
- 不把 API key 写入配置文件。
- 不保证所有 OpenAI-compatible 网关的非标准扩展一次性兼容。

## Decisions

### 1. 增加 `AiRuntime` 网关而不是让 CLI 直接调用 provider

`AiRuntime` 接收 `AppConfig`、workspace 路径和 `MetadataStore`，对外提供 `answer`、`describe_image`、`summarize_chunks`、`embed_texts`。它在调用 provider 前后统一处理：

- provider descriptor 校验；
- dry-run；
- API key 环境变量读取；
- 脱敏；
- token 估算；
- cache key 读取/写入；
- `ai_calls` 审计记录；
- 错误状态归类。

替代方案是让 `main.rs` 和 `qa.rs` 继续直接创建 provider。这个方案代码少，但会让安全和审计逻辑分散，后续 MCP/skill 调用也容易绕过同一套限制。

### 2. 将 provider 实现拆分为小模块

保留 `src/ai/mod.rs` 暴露公共 trait 和类型，新增：

- `src/ai/runtime.rs`：统一调用网关；
- `src/ai/http.rs`：共享 blocking HTTP client、错误映射和 JSON helper；
- `src/ai/ollama.rs`：Ollama 请求/响应；
- `src/ai/openai.rs`：OpenAI-compatible 请求/响应；
- `src/ai/local_http.rs`：通用本地 HTTP provider。

这样可以避免 `src/ai/mod.rs` 继续膨胀，也方便独立测试每种 provider 的请求构造。

### 3. Ollama 优先采用本地原生 API

Ollama 的 chat/vision 使用 `/api/chat`，embedding 使用 `/api/embeddings` 或当前 Ollama 支持的等价 endpoint。图片输入只发送 base64 和 hash 审计元数据，不在日志中保存原图或完整 base64。

替代方案是把 Ollama 伪装成 OpenAI-compatible。这个方式在某些部署下可用，但会掩盖 Ollama 原生 vision/embedding 差异，不利于错误诊断。

### 4. 本地 provider 强制 localhost

`ollama` 和 `local-http` 必须使用 `http://localhost:*`、`http://127.0.0.1:*` 或 `http://[::1]:*`。如果用户要连接远程企业网关，应使用 `openai-compatible` 或以后新增的企业 provider，并显式通过环境变量提供密钥。

这个决策偏保守，但符合当前项目“默认安全、本地优先”的定位。

### 5. Token 预算先做估算和截断，不做精确 tokenizer

第一版使用轻量估算：中文和英文统一按字符数近似，结合 `context_chunks`、`chunk_char_limit` 和 provider 请求级预算控制。精确 tokenizer 后续可以按 provider 插件化，但不作为本 change 的前置依赖。

### 6. 审计记录保持元数据级

`ai_calls` 继续保存 hash、provider、model、purpose、status、token_estimate、redaction_applied、output_hash 和错误类别。完整 prompt、chunk 正文、图片 base64、API key 和模型完整输出不写入审计表。

## Risks / Trade-offs

- Ollama 版本差异导致 endpoint 或响应字段不一致 → 把请求构造和响应解析放在 `ollama.rs`，并用 fixture 测试隔离兼容点。
- 本地 HTTP 网关协议不统一 → `local-http` 只承诺最小 JSON 协议，其他协议通过后续 provider 增量支持。
- token 估算不精确 → 第一版用保守截断和配置上限，后续再引入 tokenizer。
- 真实 provider 测试容易依赖本机环境 → 单元测试只验证请求构造、响应解析和安全校验；真实端到端测试放在可选手动检查或 mock HTTP server。
- HTTP 调用可能泄露敏感内容 → 默认 provider 仍为 `mock`；外部 provider 必须显式配置；本地 provider 强制 localhost；调用前应用脱敏和预算。

## Migration Plan

1. 保留现有 `mock` 行为，确保默认用户无感升级。
2. 在 `AppConfig` 中保持现有字段兼容，不移动 `.learnBusiness/config/app.toml`。
3. 新增 runtime 和 provider 模块后，先让 `ask` 和 `describe-image` 使用 runtime；报告/摘要和 embedding 后续接入同一 runtime。
4. 更新中文文档和 OpenSpec tasks，明确真实本地模型接入步骤。
5. 如果新 provider 调用失败，回退路径是把 `[ai].provider` 改回 `mock`，本地索引和报告能力不受影响。

## Open Questions

- Ollama embedding endpoint 应优先支持 `/api/embeddings` 还是兼容新版 `/api/embed`。
- 通用 `local-http` 的最小协议是否采用 OpenAI-compatible JSON，还是定义 learnBusiness 自有轻量协议。
- 是否在本 change 中加入 mock HTTP server 作为 dev-dependency，还是先用纯请求构造/响应解析单元测试。
