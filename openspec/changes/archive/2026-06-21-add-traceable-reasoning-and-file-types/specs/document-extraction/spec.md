## ADDED Requirements

### Requirement: 结构化和表格文件识别
系统 SHALL 识别 CSV、TSV、JSON、HTML、XML、YAML、YML 和 XLSX 文件，并将其纳入文档发现结果。

#### Scenario: 发现新增轻量文件类型
- **WHEN** 文档目录包含 `.csv`、`.tsv`、`.json`、`.html`、`.xml`、`.yaml`、`.yml` 或 `.xlsx`
- **THEN** `discover` MUST 返回对应文档，并设置可区分的 file_type。

### Requirement: 结构化文本文件抽取
系统 SHALL 对 CSV、TSV、JSON、HTML、XML、YAML 和 YML 执行轻量文本抽取，并写入可检索 chunk。

#### Scenario: CSV 和 TSV 可检索
- **WHEN** 用户导入包含业务字段和值的 `.csv` 或 `.tsv`
- **THEN** 系统 MUST 将表头和值抽取为可检索文本 chunk。

#### Scenario: JSON 可检索
- **WHEN** 用户导入包含业务对象和字段值的 `.json`
- **THEN** 系统 MUST 将字段名和字符串/数值/布尔值抽取为可检索文本 chunk。

#### Scenario: HTML XML YAML 可检索
- **WHEN** 用户导入 `.html`、`.xml`、`.yaml` 或 `.yml`
- **THEN** 系统 MUST 提取可读文本和值，并写入本地 chunk 索引。

### Requirement: XLSX 表格抽取
系统 SHALL 对 XLSX 文件执行轻量抽取，读取共享字符串、inline 字符串和数值，并按工作表生成 table chunk。

#### Scenario: XLSX 单元格文本可检索
- **WHEN** 用户导入包含业务表格文本的 `.xlsx`
- **THEN** 系统 MUST 生成 `kind=table` 的 chunk，且可通过 `search` 命中单元格文本。
