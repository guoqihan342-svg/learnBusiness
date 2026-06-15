# 提案：优化 AI Provider Runtime

## 背景

learnBusiness 需要在业务文档问答、图片理解、摘要和 embedding 场景中接入真实 AI 接口。早期设计区分了多个 provider 名称，容易把 `localhost` 误解为“本地部署大模型”。当前归档后的目标口径是：保留默认 `mock`，真实调用统一通过通用 `http` provider 接入。

## 目标

- 使用 `provider = "http"` 表示可配置 HTTP AI 接口。
- `base_url` 完全可配置，可指向 localhost、企业网关或云端接口。
- 认证和网关参数通过 `[ai.headers]` 配置，请求头值支持 `${ENV_NAME}`。
- 文本问答、embedding 和多模态图片请求复用同一套 `base_url` 和 headers。
- 所有 AI 调用继续经过 `AiRuntime`，统一处理脱敏、token 估算、审计、trace 和缓存。

## 非目标

- 不在配置文件中保存真实密钥值。
- 不绕过 `AiRuntime` 直接从 CLI 调 provider。
- 不把 `localhost` 绑定为本地模型语义。

## 风险与缓解

- 风险：配置头部可能包含敏感值。缓解：文档推荐 `${ENV_NAME}`，日志和审计不记录 header 值。
- 风险：不同 HTTP 服务协议不完全兼容。缓解：当前默认 chat completions/embeddings 兼容 JSON 形状，后续通过 adapter 扩展。
- 风险：远程调用泄漏业务内容。缓解：远程 HTTP provider 默认启用脱敏，并且只发送 top-k chunk。
