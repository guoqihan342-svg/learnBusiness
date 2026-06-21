## ADDED Requirements

### Requirement: search 写入检索步骤日志
系统 SHALL 在 `search` 命令执行时写入本地检索步骤日志，记录查询 hash、limit、命中数量和 trace id。

#### Scenario: search 日志不调用 AI
- **WHEN** 用户执行 `search --workspace <workspace> --limit 5 <query>`
- **THEN** 系统 MUST 写入 search 步骤日志，且 MUST NOT 构造 AI provider 或写入 `ai_calls`。

#### Scenario: search 无命中仍可追踪
- **WHEN** 用户执行 `search` 但没有任何命中
- **THEN** 系统 MUST 写入 `status=completed` 且 `result_count=0` 的步骤日志。
