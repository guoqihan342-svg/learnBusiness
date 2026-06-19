## 1. 图片描述入库

- [x] 1.1 增加 `IngestOptions` 和图片描述导入测试。
- [x] 1.2 实现 `ingest --describe-images`，将非 dry-run 图片描述写入 AI 生成 chunk。
- [x] 1.3 实现 `ingest --describe-images --dry-run-ai`，只写审计和 trace，不写描述 chunk。

## 2. 本地检索调试

- [x] 2.1 增加 `search` CLI 测试，验证输出 chunk、score、snippet 和来源元数据。
- [x] 2.2 实现 `search --workspace <workspace> <query> [--limit N]`，且不调用 AI。
- [x] 2.3 将 `search` 纳入权限策略。

## 3. 业务分析报告

- [x] 3.1 增加报告测试，覆盖业务对象、流程、规则、风险和来源引用。
- [x] 3.2 实现本地业务要素提取，增强报告 Markdown 输出。
- [x] 3.3 确保报告输出候选线索，不调用 AI，不伪造无来源结论。

## 4. 文档、归档和验证

- [x] 4.1 更新 README、操作手册、数据文档和架构文档。
- [x] 4.2 运行 `cargo fmt -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`。
- [x] 4.3 运行 `openspec validate --all`，归档 OpenSpec 变更并再次校验。
