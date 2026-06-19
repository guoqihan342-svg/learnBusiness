## ADDED Requirements

### Requirement: 本地 search 命令
系统 SHALL 提供本地 `search` 命令，用于查看索引命中结果，且该命令不得调用 AI provider。

#### Scenario: search 输出检索证据
- **WHEN** 用户执行 `search --workspace <workspace> <query>`
- **THEN** CLI MUST 输出命中 chunk 的文件路径、chunk id、score、snippet，以及存在时的页码或幻灯片编号。

#### Scenario: search 不调用 AI
- **WHEN** 用户执行 `search`
- **THEN** 系统 MUST NOT 构造或调用 AI provider，也 MUST NOT 写入 AI 调用审计。

### Requirement: search 结果数量可控
系统 SHALL 允许用户通过 `--limit` 控制 search 输出数量，并对无效数量做有界处理。

#### Scenario: search 使用 limit
- **WHEN** 用户执行 `search --limit 2`
- **THEN** CLI MUST 至多输出 2 条检索结果。

#### Scenario: limit 为 0
- **WHEN** 用户执行 `search --limit 0`
- **THEN** CLI MUST 返回没有命中结果，且 MUST NOT 调用 AI。
