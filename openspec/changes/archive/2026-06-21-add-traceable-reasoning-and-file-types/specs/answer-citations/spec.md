## ADDED Requirements

### Requirement: 问答返回推算过程摘要
系统 SHALL 在问答结果中返回安全的推算过程摘要，描述本地检索、top-k 选择、上下文截断、脱敏判断、AI 调用状态和引用绑定结果。

#### Scenario: 有命中时返回推算摘要
- **WHEN** `ask` 命中本地 chunk 并调用 AI provider
- **THEN** 问答结果 MUST 包含至少检索、上下文选择、AI 调用和引用绑定四类推算步骤。

#### Scenario: 无命中时返回本地短路摘要
- **WHEN** `ask` 没有命中任何本地 chunk
- **THEN** 问答结果 MUST 包含本地检索未命中和未调用 AI 的推算步骤。

### Requirement: 推算过程摘要不增加 AI token
系统 SHALL 使用本地运行元数据生成推算过程摘要，不得为了生成摘要额外调用 AI provider 或增加发送给 provider 的上下文正文。

#### Scenario: 推算摘要来自本地元数据
- **WHEN** 用户执行 `ask`
- **THEN** 推算摘要 MUST 由 trace id、命中数量、选中数量、截断上限、脱敏状态、token 估算和引用数量等本地元数据组成。

### Requirement: CLI 展示推算过程
系统 SHALL 在 `ask` CLI 输出中展示安全的推算过程摘要，帮助用户定位答案生成路径。

#### Scenario: ask 输出推算过程块
- **WHEN** 用户执行 `ask --workspace <workspace> <question>`
- **THEN** CLI MUST 输出“推算过程”块，并列出每个步骤的名称和安全摘要。
