## Purpose

归档 learnBusiness AI provider runtime 的优化结果：默认 `mock` 不出网，真实 AI 调用统一通过可配置 `http` provider，并复用 `AiRuntime` 的安全、审计、缓存和追踪能力。

## Requirements

### Requirement: Provider 入口收敛
系统 SHALL 使用 `mock` 和 `http` 作为主要 provider 入口。

#### Scenario: mock 不出网
- **WHEN** `[ai].provider = "mock"`
- **THEN** 系统 MUST 不执行外部网络请求。

#### Scenario: http 使用配置
- **WHEN** `[ai].provider = "http"`
- **THEN** 系统 MUST 使用配置的 `base_url`、模型名和 `[ai.headers]`。

### Requirement: base_url 可配置
系统 SHALL 允许 HTTP provider 使用合法 `http` 或 `https` `base_url`。

#### Scenario: localhost 可用
- **WHEN** `base_url = "http://localhost:8000/v1"`
- **THEN** 系统 MUST 允许创建 provider descriptor。

#### Scenario: 远程 HTTPS 可用
- **WHEN** `base_url = "https://gateway.example.com/v1"`
- **THEN** 系统 MUST 允许创建 provider descriptor。

### Requirement: 请求头复用
系统 SHALL 在问答、embedding 和多模态请求中复用同一套 `[ai.headers]`。

#### Scenario: 多模态请求带请求头
- **WHEN** 执行非 dry-run 的 `describe-image`
- **THEN** 系统 MUST 给图片理解 HTTP 请求附加配置请求头。

#### Scenario: 请求头值不入日志
- **WHEN** 写入审计或 trace
- **THEN** 系统 MUST NOT 保存请求头值。
