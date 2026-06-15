# learnBusiness

`learnBusiness` 是一个本地优先、轻量、省 token 的业务文档理解智能体。它面向一批本地业务文档运行，支持初始化工作区、导入文本和基础 PDF、建立 SQLite/全文索引、基于索引问答、生成基础报告，并通过统一的 `AiRuntime` 接入 `mock` 或通用 `http` AI provider，保留 skill 和 MCP 扩展点。

这里的 `http` provider 只表示“通过可配置 HTTP 接口调用 AI 服务”。`base_url` 可以是 `http://localhost:8000/v1`，也可以是企业网关或云端兼容接口；`localhost` 只是地址，不代表必须本地部署大模型。

## 快速开始

```powershell
cargo run --bin learnBusiness -- init .\workspace
cargo run --bin learnBusiness -- ingest .\docs --workspace .\workspace
cargo run --bin learnBusiness -- ask --workspace .\workspace "这个业务的核心流程是什么？"
cargo run --bin learnBusiness -- report --workspace .\workspace --out report.md
```

## 工作区

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
- 轻量分片：长文本按配置上限切成小 chunk，避免问答上下文过大。
- 本地索引：使用 SQLite 保存文档、chunk 和 AI 调用记录，并使用 FTS5 做全文检索。
- 问答：只取配置指定数量的相关 chunk 调用 AI provider，并输出来源引用。
- 报告：生成包含执行摘要、资料集概览、流程候选和来源引用的 Markdown 报告。
- 图片理解：`describe-image` 通过 `AiRuntime` 调用多模态 HTTP 接口；`--dry-run-ai` 只记录调用计划，不发送图片。

## 安全与省 Token

- 默认 provider 是 `mock`，不执行外部网络请求。
- 配置文件不保存密钥值；推荐在 `[ai.headers]` 中使用 `${ENV_NAME}` 占位符，例如 `Authorization = "Bearer ${LEARNBUSINESS_AI_KEY}"`。
- `http` provider 的文本、embedding、多模态图片请求复用同一套 `base_url` 和请求头配置。
- 远程 HTTP provider 调用前默认执行脱敏；loopback `base_url` 视为本机端点，但不等同“本地模型”。
- 审计、缓存和 trace 只保存 provider、model、purpose、hash、状态、token 估算、失败类别和脱敏标记，不保存完整 prompt、图片正文、上下文正文或请求头值。
- 问答只发送 `performance.context_chunks` 指定的 top-k chunk，默认 5，最大 20。

## AI Provider 配置

```toml
[ai]
provider = "http"
base_url = "http://localhost:8000/v1"
chat_model = "business-chat"
vision_model = "business-vision"
embedding_model = "business-embedding"
api_key_env = ""

[ai.headers]
Authorization = "Bearer ${LEARNBUSINESS_AI_KEY}"
X-App = "learnBusiness"
```

| provider | 用途 | 说明 |
| --- | --- | --- |
| `mock` | 离线验证索引、问答、图片 dry-run 和审计链路 | 不出网，不需要密钥 |
| `http` | 调用可配置 HTTP AI 接口 | `base_url` 和请求头均由配置决定；问答、embedding、多模态复用同一 headers |

`api_key_env` 仍作为兼容快捷方式保留：如果没有显式配置 `Authorization` 请求头，但设置了 `api_key_env`，运行时会自动生成 `Authorization: Bearer <环境变量值>`。

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
