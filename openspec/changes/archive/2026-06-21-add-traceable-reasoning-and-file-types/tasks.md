## 1. 步骤日志与 trace inspection

- [x] 1.1 为步骤日志和 trace 查询编写失败测试，覆盖 ask/search/ingest 主路径和敏感正文不落日志。
- [x] 1.2 实现 OperationTraceLogger、OperationTraceEvent、trace id 生成和 `.learnBusiness/logs/operations.jsonl` 路径。
- [x] 1.3 在 search、ingest、ask、describe-image 主路径写入关键步骤事件，并复用 `[logging].trace_enabled`。
- [x] 1.4 新增 `inspect-trace` CLI，支持最近事件和 `--trace <trace_id>` 过滤。

## 2. 问答推算过程摘要

- [x] 2.1 为 `QaAnswer` 推算过程摘要编写失败测试，覆盖有命中、无命中、token 估算和脱敏状态。
- [x] 2.2 扩展 `AiRuntime::answer` 返回安全推算元数据，并确保不额外调用 AI。
- [x] 2.3 更新 `ask` CLI 输出“推算过程”块，并保留现有答案和来源输出。

## 3. 新增文件类型识别与抽取

- [x] 3.1 为 CSV、TSV、JSON、HTML、XML、YAML/YML、XLSX 的 discover/extract/ingest 编写失败测试。
- [x] 3.2 扩展 `discover` 文件类型识别和 MIME 映射。
- [x] 3.3 实现 CSV、TSV、JSON、HTML、XML、YAML/YML 的轻量文本抽取。
- [x] 3.4 实现 XLSX 共享字符串、inline 字符串和数值抽取，并写入 `kind=table` chunk。

## 4. 文档、验证与收尾

- [x] 4.1 更新 README、操作手册、数据文档、架构文档，说明推算过程、步骤日志、安全边界和新增文件类型。
- [x] 4.2 运行 `cargo fmt -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、`openspec validate --all`、`git diff --check` 直到全部通过。
- [x] 4.3 归档 OpenSpec 变更，确认主 specs 更新后再次完整验证。
- [x] 4.4 提交并推送到 GitHub。
