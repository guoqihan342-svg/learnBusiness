# execution-permissions Specification

## Purpose
TBD - created by archiving change enhance-document-extraction-citations-observability. Update Purpose after archive.
## Requirements
### Requirement: CLI 命令声明权限
系统 SHALL 为每个 CLI 命令声明所需权限，并在执行命令逻辑前统一校验。

#### Scenario: init 需要写工作区权限
- **WHEN** 用户执行 `init`
- **THEN** 系统 MUST 校验 `WriteWorkspace` 权限。

#### Scenario: ingest 需要读本地和写工作区权限
- **WHEN** 用户执行 `ingest`
- **THEN** 系统 MUST 校验 `ReadLocal` 和 `WriteWorkspace` 权限。

#### Scenario: describe-image 非 dry-run 需要外部 AI 权限
- **WHEN** 用户执行非 dry-run 的 `describe-image` 且 provider 可能外呼
- **THEN** 系统 MUST 校验 `AiExternal`，必要时校验 `ExternalNetwork`。

### Requirement: 权限失败安全退出
系统 SHALL 在权限缺失时返回明确错误，并且不得执行对应文件写入、外部网络或 AI provider 调用。

#### Scenario: 缺少权限时不执行命令主体
- **WHEN** 命令所需权限未被授予
- **THEN** 系统 MUST 返回缺失权限错误，并且 MUST NOT 执行命令主体。

### Requirement: 权限模型预留 MCP 接入
系统 SHALL 保留 `McpExternal` 权限用于后续 MCP 工具接入，避免 MCP 能力绕过统一授权边界。

#### Scenario: MCP 工具声明外部权限
- **WHEN** 后续 MCP 工具被注册为外部工具
- **THEN** 系统 MUST 能够使用 `McpExternal` 权限表达该工具的授权要求。

