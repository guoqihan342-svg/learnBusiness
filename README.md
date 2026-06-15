# learnBusiness

`learnBusiness` 是一个本地优先、轻量、省 token 的业务理解智能体。它面向一批本地业务文档运行，支持初始化工作区、导入文本和基础 PDF、建立 SQLite/全文索引、基于索引问答、生成基础报告，并预留多模态 AI provider、skill 和 MCP 扩展点。

## 快速开始

```powershell
cargo run --bin learnBusiness -- init .\workspace
cargo run --bin learnBusiness -- ingest .\docs --workspace .\workspace
cargo run --bin learnBusiness -- ask --workspace .\workspace "这个业务的核心流程是什么？"
cargo run --bin learnBusiness -- report --workspace .\workspace --out report.md
```

## 本地工作区

`init` 会在目标工作区下创建 `.learnBusiness/`。配置集中写入 `.learnBusiness/config/app.toml`，索引、缓存、artifact 和日志继续分目录保存：

```text
.learnBusiness/
  config/
    app.toml
  metadata.sqlite
  fulltext/
  vectors/
  artifacts/
  cache/
  logs/
```

`.learnBusiness/` 已加入 `.gitignore`，避免把本地配置、AI 缓存、日志或潜在敏感索引提交到仓库。

## 当前能力

- 本地工作区：配置集中在 `config/`，数据和缓存按用途隔离。
- 文档发现：支持 `txt`、`md`、`pdf`、常见图片、`docx`、`pptx` 的类型识别和 hash。
- 文本抽取：支持纯文本、Markdown 和基础 PDF 文本抽取。
- 轻量分片：长文本按固定上限切成小 chunk，避免问答上下文过大。
- 本地索引：使用 SQLite 保存文档、chunk 和 AI 调用记录，并使用 FTS5 做全文检索。
- 问答：只取少量相关 chunk 调用配置指定的 AI provider，并输出来源引用。
- 报告：生成包含执行摘要、资料集概览、流程候选和来源引用的 Markdown 报告。
- 图片 dry-run：`describe-image --dry-run-ai` 可显示将发送给 AI 的图片 hash 和 MIME 类型。

## 安全和省 token 策略

- 默认不发送原始文件到外部服务。
- 配置文件不保存 API key；真实密钥应走环境变量或外部密钥管理。
- AI provider 当前支持 `mock`、`openai-compatible`、`ollama`、`local-http` 四类配置入口；本地 provider 仅允许 localhost 地址，外部 provider 缺少 API key 会返回明确错误。
- `describe-image --dry-run-ai` 只展示调用计划，不执行 AI 调用。
- 未变化文件按内容 hash 跳过，避免重复抽取和重复 AI 调用。
- 问答只发送配置指定数量的 top-k 相关 chunk，默认 5，避免把整份文档塞进上下文。
- 脱敏模块已覆盖邮箱、中国大陆手机号、长数字和 `sk-` 样式密钥。

## 文档

- [操作手册](docs/operation-manual.md)：安装、初始化、导入、问答、报告、图片 dry-run、配置和排障。
- [数据文档](docs/data-documentation.md)：工作区目录、SQLite 表、FTS、缓存、生命周期和隐私边界。
- [架构文档](docs/architecture.md)：模块职责、数据流、安全边界、扩展点和性能策略。
- [设计说明](docs/superpowers/specs/2026-06-14-learnBusiness-design.md)：第一版设计目标和边界。
- [实现计划](docs/superpowers/plans/2026-06-14-learnBusiness.md)：已完成能力和后续增强方向。

## 开发验证

```powershell
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```
