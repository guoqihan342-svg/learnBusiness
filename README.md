# biz-agent

`biz-agent` 是一个本地优先的业务文档理解 agent。第一版支持初始化工作区、ingest 文本和基础 PDF、建立 SQLite/全文索引、基于索引问答、生成基础报告，并预留多模态 AI provider、skill 和 MCP 扩展点。

## 快速开始

```powershell
cargo run -- init .\workspace
cargo run -- ingest .\docs --workspace .\workspace
cargo run -- ask --workspace .\workspace "这个业务的核心流程是什么？"
cargo run -- report --workspace .\workspace --out report.md
```

## 当前能力

- 本地工作区：创建 `.agent-index`、配置、缓存、artifact 和日志目录。
- 文档发现：支持 `txt`、`md`、`pdf`、常见图片、`docx`、`pptx` 的类型识别和 hash。
- 文本抽取：支持纯文本、Markdown 和基础 PDF 文本抽取。
- 本地索引：使用 SQLite 保存文档、chunk 和 AI 调用记录，并使用 FTS5 做全文检索。
- 问答：基于本地检索结果调用 mock AI provider，输出来源引用。
- 报告：生成包含执行摘要、资料集概览、流程候选和来源引用的 Markdown 报告。
- 图片 dry-run：`describe-image --dry-run-ai` 可显示将发送给 AI 的图片 hash 和 MIME 类型。

## 安全边界

- 默认不发送原始文件到外部服务。
- 外部 AI provider 当前只有骨架，缺少 API key 会返回明确错误。
- `describe-image --dry-run-ai` 只展示调用计划，不执行 AI 调用。
- 脱敏模块已覆盖邮箱、中国大陆手机号、长数字和 `sk-` 样式密钥。

## 开发验证

```powershell
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```
