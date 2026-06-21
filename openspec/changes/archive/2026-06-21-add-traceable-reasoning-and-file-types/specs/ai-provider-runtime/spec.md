## ADDED Requirements

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
