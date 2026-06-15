# 设计：通用 HTTP AI Provider Runtime

## 概览

AI 调用由 `AiRuntime` 统一入口管理。Provider registry 保留两个主要入口：

- `mock`：默认离线 provider，不出网。
- `http`：通用 HTTP AI 接口，通过配置决定 `base_url`、模型名和请求头。

旧的 `openai-compatible` 字符串作为兼容别名解析到 `http`。新文档和配置不再推荐使用 provider 专名表达厂商或本地模型。

## 配置

```toml
[ai]
provider = "http"
base_url = "http://localhost:8000/v1"
chat_model = "business-chat"
vision_model = "business-vision"
embedding_model = "business-embedding"
api_key_env = ""

[ai.headers]
Authorization = "Bearer ${LEARNBUSINESS_AI_KEY}"
X-App = "learnBusiness"
```

`api_key_env` 是兼容快捷方式：未配置 `Authorization` 时，可用它生成 bearer token。推荐新配置使用 `[ai.headers]`。

## Runtime 边界

`AiRuntime` 负责：

- 配置加载和 provider descriptor。
- top-k chunk 和 chunk 长度控制。
- loopback/remote endpoint 判断。
- 远程 HTTP 调用前脱敏。
- token 估算。
- `ai_calls` 审计。
- `trace.jsonl` 追踪。
- AI cache 写入。

## HTTP 请求

`HttpAiProvider` 当前使用 chat completions/embeddings 兼容 JSON 形状：

- `POST {base_url}/chat/completions`：问答、摘要、图片理解。
- `POST {base_url}/embeddings`：embedding。

文本、embedding 和多模态请求都调用 `headers_from_config`，展开 `${ENV_NAME}` 并校验 HeaderName/HeaderValue。缺失环境变量会在网络请求前失败。

## 安全

- 配置文件不保存真实密钥值。
- 日志、审计和缓存 key 不记录请求头值。
- HTTP client 默认关闭环境代理，避免 loopback 请求被代理劫持。
- `base_url` 只要求合法 `http` 或 `https`，不再按 provider 名称强制 localhost。
