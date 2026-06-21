## ADDED Requirements

### Requirement: 结构化步骤日志
系统 SHALL 为主命令路径写入本地结构化步骤日志，用于追踪命令、组件、步骤、状态、trace id、输入 hash、输出 hash、命中数量、token 估算、错误分类和耗时。

#### Scenario: ask 写入步骤日志
- **WHEN** 用户执行 `ask` 且本地索引命中至少一个 chunk
- **THEN** 系统 MUST 写入包含检索、上下文选择、AI 调用和引用绑定步骤的结构化日志事件。

#### Scenario: ingest 写入步骤日志
- **WHEN** 用户执行 `ingest`
- **THEN** 系统 MUST 写入发现文档、抽取文档、写入索引和完成汇总步骤的结构化日志事件。

#### Scenario: search 写入步骤日志
- **WHEN** 用户执行 `search`
- **THEN** 系统 MUST 写入本地检索步骤日志，且 MUST NOT 写入 AI 调用审计。

### Requirement: 步骤日志不保存敏感正文
系统 SHALL 禁止在步骤日志中保存完整 prompt、完整业务正文、图片 base64、HTTP 请求头值、API key 或 provider 完整返回体。

#### Scenario: 敏感文本不会进入日志
- **WHEN** 用户的问题或文档正文包含手机号、邮箱、API key 或长业务正文
- **THEN** `.learnBusiness/logs/operations.jsonl` MUST NOT 包含这些完整原文值，只能包含 hash、数量、状态和短元数据。

#### Scenario: 请求头不会进入日志
- **WHEN** HTTP provider 使用 `[ai.headers]`
- **THEN** 步骤日志 MUST NOT 包含任何请求头真实值或环境变量展开后的 token。

### Requirement: Trace inspection
系统 SHALL 提供 CLI 能力查看步骤日志，并支持按 trace id 过滤。

#### Scenario: 查看最近步骤日志
- **WHEN** 用户执行 `inspect-trace --workspace <workspace>`
- **THEN** CLI MUST 输出最近步骤事件的 trace id、component、operation、step、status 和可安全展示的统计字段。

#### Scenario: 按 trace id 过滤步骤日志
- **WHEN** 用户执行 `inspect-trace --workspace <workspace> --trace <trace_id>`
- **THEN** CLI MUST 只输出匹配该 trace id 的步骤事件。

### Requirement: 步骤日志可关闭
系统 SHALL 复用 `[logging].trace_enabled` 控制步骤日志写入。

#### Scenario: 禁用 trace 时不写步骤日志
- **WHEN** `[logging].trace_enabled = false`
- **THEN** 系统 MUST NOT 创建或追加 `.learnBusiness/logs/operations.jsonl`。
