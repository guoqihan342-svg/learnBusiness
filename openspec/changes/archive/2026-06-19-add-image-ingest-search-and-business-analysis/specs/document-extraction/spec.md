## ADDED Requirements

### Requirement: 显式图片描述入库
系统 SHALL 仅在用户显式开启图片描述导入时，调用 AI provider 生成图片描述，并将描述作为可检索 chunk 写入索引。

#### Scenario: 默认导入图片不外呼 AI
- **WHEN** 用户执行 `ingest` 且未传入图片描述选项
- **THEN** 系统 MUST 只登记图片文档，且 MUST NOT 调用 AI provider。

#### Scenario: 图片描述写入索引
- **WHEN** 用户执行 `ingest --describe-images` 且图片描述 provider 调用成功
- **THEN** 系统 MUST 写入一个 `ai_generated = true` 的图片描述 chunk，并记录图片 artifact 路径。

#### Scenario: 图片描述 dry-run 不写 chunk
- **WHEN** 用户执行 `ingest --describe-images --dry-run-ai`
- **THEN** 系统 MUST 记录 AI dry-run 审计和 trace，且 MUST NOT 写入图片描述 chunk。
