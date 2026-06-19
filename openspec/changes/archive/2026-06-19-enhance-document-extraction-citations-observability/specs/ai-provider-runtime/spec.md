## ADDED Requirements

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
