# learnBusiness 实现计划

## 目标

构建一个 Rust CLI 纵向切片，支持本地业务文档目录初始化、文件发现、文本/PDF 抽取、SQLite 元数据和全文检索、mock AI、通用 HTTP AI provider、图片描述入口、问答和基础报告。

项目对外名称和 CLI 二进制为 `learnBusiness`，Cargo 包名为 `learn-business`，lib crate 名为 `learn_business`。

## 核心原则

- 本地优先：默认不上传原始文档。
- 配置集中：运行时配置写入 `.learnBusiness/config/app.toml`。
- 轻量：不强制引入外部向量数据库或服务端。
- 省 token：未变文件跳过、长文档切分、问答只取 top-k chunk。
- 安全：密钥值不写入配置，AI/MCP 通过显式配置和审计链路接入。

## 当前文件结构

- `src/main.rs`：CLI 入口和命令分发。
- `src/config.rs`：配置模型、默认值、AI headers、性能和日志配置。
- `src/workspace.rs`：`.learnBusiness` 工作区初始化和路径管理。
- `src/discover.rs`：文件发现、类型识别和 SHA-256 hash。
- `src/store.rs`：SQLite schema、元数据写入、FTS5 全文搜索。
- `src/ingest/`：ingest pipeline 和文本/PDF 抽取。
- `src/ai/mod.rs`：`AiProvider` trait、mock provider、provider descriptor、HTTP headers 解析。
- `src/ai/http_provider.rs`：通用 HTTP AI provider，当前使用 chat completions/embeddings 兼容 JSON 形状。
- `src/ai/http.rs`：共享 HTTP JSON 调用、URL helper、请求头校验。
- `src/ai/cache.rs`：AI cache key。
- `src/ai/redaction.rs`：基础脱敏。
- `src/ai/runtime.rs`：统一 AI 调用网关、审计、trace、缓存。
- `src/qa.rs`：基于本地检索的问答。
- `src/report.rs`：基础业务理解报告。
- `src/task.rs`：任务、agent、tool 和权限模型。
- `tests/cli_flow.rs`：端到端 CLI 流程测试。

## 已完成能力

- [x] 初始化 Rust crate 和 CLI 外壳。
- [x] 建立模型、权限和任务抽象。
- [x] 初始化 `.learnBusiness` 本地工作区。
- [x] 将运行时配置集中到 `.learnBusiness/config/app.toml`。
- [x] 支持 `[ai.headers]` 请求头配置和 `${ENV_NAME}` 占位符。
- [x] 发现支持的本地文档并计算 hash。
- [x] 抽取文本、Markdown 和基础 PDF。
- [x] 写入 SQLite 元数据和 FTS5 全文索引。
- [x] 未变化文件重复 ingest 时跳过。
- [x] 文档变化后清理旧 chunk，避免旧内容残留。
- [x] 长文本按固定上限切成小 chunk。
- [x] 加入 AI provider trait、mock provider、通用 HTTP provider、缓存 key 和脱敏模块。
- [x] `ask` 只基于命中 chunk 调用 AI；无命中时不发送业务内容。
- [x] `describe-image --dry-run-ai` 写入 AI 调用审计记录。
- [x] 多模态 HTTP 请求复用同一套 `base_url` 和 headers。
- [x] 生成中文业务理解报告。
- [x] `.learnBusiness/` 加入 `.gitignore`。

## 后续增强

- [ ] 加入 DOCX/PPTX 深度抽取。
- [ ] 加入 OCR 或页面截图理解。
- [ ] 增加向量索引或轻量 embedding cache。
- [ ] 为不同 HTTP 协议增加 adapter，而不是绕过 `AiRuntime`。
- [ ] 接入 MCP server 前增加权限白名单和 dry-run 预览。
- [ ] 让 AI 审计记录包含更准确的 token 估算。

## 验证命令

```powershell
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## 验收标准

- `learnBusiness --help` 显示核心命令：`ingest`、`ask`、`report`。
- `learnBusiness init <workspace>` 创建 `.learnBusiness/config/app.toml`。
- `.learnBusiness/config.toml` 不再出现。
- 导入文本后生成 `.learnBusiness/metadata.sqlite`。
- 长文本会分成多个不超过配置上限的 chunk。
- `provider=http` 支持可配置 `base_url` 和请求头。
- 文档和 README 均使用中文并统一项目名 `learnBusiness`。
