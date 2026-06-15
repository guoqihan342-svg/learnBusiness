## 1. Provider 运行时边界

- [x] 1.1 为 `AiProviderDescriptor` 增加单元测试，覆盖 `mock`、`openai-compatible`、`ollama`、`local-http`、未知 provider、本地远程 URL 拒绝。
- [x] 1.2 将 `src/ai/mod.rs` 拆分为 `runtime.rs`、`http.rs`、`ollama.rs`、`openai.rs`、`local_http.rs`，保留 `mod.rs` 作为公共导出入口。
- [x] 1.3 新增 `AiRuntime`，统一持有 `AppConfig`、provider descriptor、workspace 路径和 provider 实例。
- [x] 1.4 将 `qa::answer_workspace` 改为调用 `AiRuntime::answer`，保持 mock 默认行为不变。
- [x] 1.5 将 `main.rs` 的 `describe-image` 改为调用 `AiRuntime::describe_image`，dry-run 和非 dry-run 共用同一审计路径。

## 2. 本地模型 Provider

- [x] 2.1 为 Ollama chat 请求构造写失败测试，验证 model、messages、上下文文本和 stream=false。
- [x] 2.2 实现 Ollama answer 请求构造和响应解析，不在日志或错误中暴露完整上下文。
- [x] 2.3 为 Ollama vision 请求构造写失败测试，验证图片 base64 只进入请求体，不进入审计记录。
- [x] 2.4 实现 Ollama describe_image 请求构造和响应解析。
- [x] 2.5 为 Ollama embedding 请求构造写失败测试，覆盖 `/api/embeddings` 或选定 endpoint。
- [x] 2.6 实现 Ollama embed_texts 请求构造和响应解析。
- [x] 2.7 为 `local-http` provider 写协议测试，定义最小 answer、describe_image、embed_texts JSON 结构。
- [x] 2.8 实现 `local-http` provider 的最小协议调用骨架和响应解析。

## 3. OpenAI-compatible Provider

- [x] 3.1 为 OpenAI-compatible chat completion 请求构造写失败测试，验证 Authorization 只来自环境变量。
- [x] 3.2 实现 OpenAI-compatible answer 请求构造和响应解析。
- [x] 3.3 为 OpenAI-compatible vision 请求构造写失败测试，验证图片输入以受控 payload 发送。
- [x] 3.4 实现 OpenAI-compatible describe_image 请求构造和响应解析。
- [x] 3.5 为 OpenAI-compatible embedding 请求构造写失败测试。
- [x] 3.6 实现 OpenAI-compatible embed_texts 请求构造和响应解析。

## 4. 安全、Token 和审计

- [x] 4.1 新增轻量 token 估算函数和测试，覆盖中文、英文、空文本和超长文本。
- [x] 4.2 在 `AiRuntime` 中应用 `performance.context_chunks` 和 `performance.chunk_char_limit`，超限时截断或返回明确错误。
- [x] 4.3 在外部 provider 调用前接入 `redact_sensitive_text`，并把 `redaction_applied` 写入 cache key 和审计记录。
- [x] 4.4 扩展 `AiCallRecord` 或新增错误字段，记录失败类别但不保存完整 prompt。
- [x] 4.5 为 provider 调用失败写回归测试，验证 `inspect-ai` 能看到失败状态和错误类别。
- [x] 4.6 为 AI cache 隔离写测试，覆盖 provider、model、purpose、content_hash、prompt_version、redaction_applied。

## 5. CLI 和文档

- [x] 5.1 更新 `describe-image --dry-run-ai` 输出，展示 provider、model、purpose、input_hash、redaction、token_estimate 和是否本地 provider。
- [x] 5.2 增加 `inspect-ai` 输出失败类别和 output_hash 的显示。
- [x] 5.3 更新 README，说明 `mock/openai-compatible/ollama/local-http` 配置和默认安全边界。
- [x] 5.4 更新 `docs/operation-manual.md`，加入 Ollama 启动、配置、dry-run、常见错误和回退到 mock 的步骤。
- [x] 5.5 更新 `docs/data-documentation.md`，说明新增审计字段、cache key 和不保存正文的约束。
- [x] 5.6 更新 `docs/architecture.md`，说明 `AiRuntime`、provider 模块拆分和本地 provider 安全边界。

## 6. 验证和发布

- [x] 6.1 运行 `cargo fmt -- --check` 并修复格式问题。
- [x] 6.2 运行 `cargo clippy --all-targets -- -D warnings` 并修复 lint。
- [x] 6.3 运行 `cargo test`，确保全部单元测试和 CLI 测试通过。
- [x] 6.4 运行 `openspec validate optimize-local-ai-provider-runtime`。
- [x] 6.5 推送实现提交，并在完成后归档 OpenSpec change。
