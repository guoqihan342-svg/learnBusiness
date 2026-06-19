## 1. 抽取和数据模型

- [x] 1.1 增加 Office Open XML 轻量抽取依赖和 `.docx/.pptx` 正文抽取测试。
- [x] 1.2 实现 `.docx` 段落文本抽取，并让导入后的正文可被 FTS 检索。
- [x] 1.3 实现 `.pptx` 幻灯片文本抽取，并把 slide 编号写入 chunk 元数据。
- [x] 1.4 保留图片和空正文文档记录，标记为待 AI/OCR 或无可索引正文状态。

## 2. 引用和检索输出

- [x] 2.1 扩展 `SearchResult` 和 `Citation`，包含 chunk id、文件路径、页码、幻灯片、source range、artifact path、score 和 snippet。
- [x] 2.2 让 `QaAnswer` 返回结构化引用，并保持无命中时不调用 AI。
- [x] 2.3 更新 `ask` CLI 输出，展示可定位来源和检索分数。

## 3. AI trace 和排障

- [x] 3.1 为 `ai_calls` 增加兼容迁移字段 `trace_id`。
- [x] 3.2 让 AI runtime 在成功、失败和 dry-run 审计中写入 trace id。
- [x] 3.3 为 `inspect-ai` 增加 trace id 输出和 `--trace <id>` 过滤。

## 4. 权限网关

- [x] 4.1 为 CLI 命令建立权限声明和统一校验 helper。
- [x] 4.2 覆盖 `init`、`ingest`、`status`、`report`、`ask`、`inspect-ai` 和 `describe-image` 权限。
- [x] 4.3 增加权限缺失时不执行命令主体的单元测试。

## 5. 文档和验证

- [x] 5.1 更新 README、操作手册、数据文档和架构文档，说明新抽取、引用、trace 和权限行为。
- [x] 5.2 运行 `cargo fmt -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`。
- [x] 5.3 运行 `openspec validate --all`，确认 OpenSpec 变更和现有 spec 均通过。
