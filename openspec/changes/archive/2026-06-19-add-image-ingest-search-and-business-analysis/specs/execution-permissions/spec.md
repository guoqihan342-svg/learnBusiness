## ADDED Requirements

### Requirement: search 命令权限
系统 SHALL 将 `search` 纳入 CLI 权限策略，且只要求本地读取权限。

#### Scenario: search 只需读权限
- **WHEN** 用户执行 `search`
- **THEN** 系统 MUST 校验 `ReadLocal` 权限，且 MUST NOT 要求 `AiExternal`。

### Requirement: 图片描述入库权限
系统 SHALL 将 `ingest --describe-images` 纳入权限策略，区分 dry-run 和真实 AI 调用。

#### Scenario: 图片描述 dry-run 权限
- **WHEN** 用户执行 `ingest --describe-images --dry-run-ai`
- **THEN** 系统 MUST 校验 `ReadLocal` 和 `WriteWorkspace`，且 MUST NOT 要求 `ExternalNetwork`。

#### Scenario: 图片描述真实调用权限
- **WHEN** 用户执行 `ingest --describe-images` 且不是 dry-run
- **THEN** 系统 MUST 校验 `ReadLocal`、`WriteWorkspace`、`AiExternal` 和 `ExternalNetwork`。
