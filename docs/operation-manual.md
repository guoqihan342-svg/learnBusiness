# learnBusiness 操作手册

## 用途

`learnBusiness` 是一个本地优先、轻量、省 token 的业务文档理解工具。它适合把一批业务资料放进本地工作区，先建立索引，再围绕资料做问答、生成基础报告，并在需要处理图片时先用 dry-run 检查 AI 调用计划。

当前版本重点服务这些场景：

- 整理本地业务资料，支持 `.txt`、`.md`、`.pdf`、`.png`、`.jpg`、`.jpeg`、`.webp`、`.docx`、`.pptx` 的发现和登记。
- 对纯文本、Markdown 和基础 PDF 文本建立本地全文索引。
- 基于索引回答问题，只取少量相关内容块作为上下文。
- 从已索引资料生成 Markdown 报告草稿。
- 对图片做 AI 识别前先 dry-run，查看 provider、模型、输入 hash、MIME 类型和调用记录。
- 把配置集中放在 `.learnBusiness/config/app.toml`，把索引、缓存、日志放在 `.learnBusiness/` 下。

当前 AI 提供方默认是 `mock`，适合离线验证工作流、索引、报告和调用审计。需要真实模型时，可以在 `.learnBusiness/config/app.toml` 中切换到 `ollama`、`local-http` 或 `openai-compatible`；运行时会统一经过 `AiRuntime` 做 provider 校验、脱敏、token 估算、审计和缓存。

## 环境准备

请先准备 Windows PowerShell 和 Rust 工具链。建议使用稳定版 Rust，并在项目根目录执行命令。

```powershell
rustc --version
cargo --version
```

进入项目目录：

```powershell
Set-Location -LiteralPath "C:\projects\learnBusiness"
```

建议把待导入资料放在工作区外的单独目录，例如：

```powershell
New-Item -ItemType Directory -Force .\samples\docs
Set-Content -Encoding UTF8 .\samples\docs\process.md "# 业务流程`n客户提交申请，运营审核，系统归档。"
```

## 构建

开发环境下推荐直接用 Cargo 运行：

```powershell
cargo run --bin learnBusiness -- --help
```

构建可执行文件：

```powershell
cargo build --bin learnBusiness
```

构建完成后，可执行文件通常位于：

```powershell
.\target\debug\learnBusiness.exe
```

如果已经把 `learnBusiness.exe` 放进 `PATH`，也可以使用短命令：

```powershell
learnBusiness --help
```

后续示例会优先使用完整写法：

```powershell
cargo run --bin learnBusiness -- ...
```

同时给出可选短写法：

```powershell
learnBusiness ...
```

## 工作区初始化

工作区是 `learnBusiness` 保存本地索引、配置、缓存和日志的目录。初始化会在目标目录下创建 `.learnBusiness/`，并写入默认配置 `.learnBusiness/config/app.toml`。

```powershell
cargo run --bin learnBusiness -- init .\workspace
```

可选短写法：

```powershell
learnBusiness init .\workspace
```

初始化后会生成类似目录：

```text
workspace\
  .learnBusiness\
    config\
      app.toml
    metadata.sqlite
    fulltext\
    vectors\
    artifacts\
      images\
      pages\
      thumbnails\
    cache\
      ai\
      extraction\
    logs\
```

重点关注配置文件：

```powershell
Get-Content -LiteralPath .\workspace\.learnBusiness\config\app.toml
```

`.learnBusiness/` 是本地运行状态目录，通常不应该提交到代码仓库。里面可能包含本地索引、AI 缓存、提取缓存、日志和资料路径。

## 导入文档

导入会扫描资料目录，识别支持的文件，计算文件内容 hash，提取可直接处理的文本，并写入本地 SQLite 和全文索引。

```powershell
cargo run --bin learnBusiness -- ingest .\samples\docs --workspace .\workspace
```

可选短写法：

```powershell
learnBusiness ingest .\samples\docs --workspace .\workspace
```

命令完成后会输出扫描、索引、跳过和警告数量。未变化文件会根据内容 hash 跳过，避免重复提取和重复处理。

当前导入行为：

- `.txt` 和 `.md` 会按文本读取。
- `.pdf` 会尝试抽取基础文本。
- 图片、`.docx`、`.pptx` 会被发现和登记为需要 AI 或后续处理的资料，但当前导入不会自动把它们的内容转成可问答文本。
- 不支持的扩展名会被忽略。
- 提取失败会计入警告，不会中断整批导入。

省 token 机制主要发生在导入和问答两个阶段：

- 文件内容 hash：相同内容重复导入时跳过，减少重复提取和后续 AI 处理。
- 文本 chunk：长文本默认按约 1600 字符切成较小内容块，避免整份文档进入上下文。
- top-k 检索：问答默认只取最相关的 5 个内容块；可通过 `.learnBusiness/config/app.toml` 的 `context_chunks` 调整，不把整批资料发送给 AI。

查看工作区状态：

```powershell
cargo run --bin learnBusiness -- status --workspace .\workspace
```

可选短写法：

```powershell
learnBusiness status --workspace .\workspace
```

## 问答

问答会先在本地全文索引里检索问题相关内容，再把少量相关 chunk 交给当前 AI 提供方生成回答。默认提供方是模拟实现，因此输出会体现调用链路和来源，但不是生产级自然语言答案。

```powershell
cargo run --bin learnBusiness -- ask --workspace .\workspace "这个业务的核心流程是什么？"
```

可选短写法：

```powershell
learnBusiness ask --workspace .\workspace "这个业务的核心流程是什么？"
```

建议提问方式：

- 尽量使用业务资料中的关键词，例如“客户准入”“审批流程”“归档规则”。
- 问题过泛时，检索可能找不到来源；可以换成更贴近文档原文的表达。
- 先导入资料，再问答；没有索引时不会有有效来源。

问答输出会包含回答和来源路径。来源路径用于回到原始资料核对，不代表整份原始文件被上传。

## 报告

报告命令会基于本地索引生成 Markdown 文件，适合做第一版业务理解草稿。

```powershell
cargo run --bin learnBusiness -- report --workspace .\workspace --out .\workspace\report.md
```

可选短写法：

```powershell
learnBusiness report --workspace .\workspace --out .\workspace\report.md
```

查看报告：

```powershell
Get-Content -LiteralPath .\workspace\report.md
```

当前报告会包含资料集概览、执行摘要、候选流程、待确认问题和来源引用。它依赖已索引 chunk，因此导入资料越充分，报告越有参考价值。报告仍需要业务人员复核，尤其是规则边界、例外流程和人工处理路径。

## 图片 dry-run

图片 dry-run 用于在真正接入外部 AI 前查看调用计划。它会计算图片 hash、识别 MIME 类型，记录一次 AI 调用审计，但不会上传图片，也不会执行真实外部 AI 调用。

```powershell
cargo run --bin learnBusiness -- describe-image .\samples\docs\flow.png --workspace .\workspace --dry-run-ai
```

可选短写法：

```powershell
learnBusiness describe-image .\samples\docs\flow.png --workspace .\workspace --dry-run-ai
```

dry-run 输出会包含：

- 调用目的：`describe_image`
- provider 和模型名
- 图片路径
- 图片内容 `sha256` / `input_hash`
- MIME 类型
- 脱敏标记
- token 估算
- 是否为本地 provider

这一步适合用来审查“哪些图片会被处理”“会使用哪个 provider”和“输入标识是什么”。如果没有 `--dry-run-ai`，当前版本会按配置调用 provider；默认 `mock` 会生成确定性的模拟描述，真实 provider 会执行对应 HTTP 调用，并把成功结果写入 `.learnBusiness/cache/ai/`。

## 查看 AI 调用

AI 调用审计记录保存在工作区的 `.learnBusiness/metadata.sqlite` 中，可通过命令查看。

```powershell
cargo run --bin learnBusiness -- inspect-ai --workspace .\workspace
```

可选短写法：

```powershell
learnBusiness inspect-ai --workspace .\workspace
```

输出字段包括：

- `purpose`：调用目的，例如 `describe_image`。
- `provider`：提供方，例如 `mock`。
- `model`：模型名，例如 `mock-ai`。
- `status`：状态，例如 `dry_run` 或 `completed`。
- `input_hash`：输入内容 hash。
- `output_hash`：模型输出 hash；dry-run 或失败时为 `-`。
- `redaction`：是否应用脱敏。
- `token_estimate`：估算 token 数。
- `error_category`：失败分类；成功或 dry-run 时为 `-`。

建议在接入真实外部 AI 前，先用 `describe-image --dry-run-ai` 和 `inspect-ai` 验证审计链路。

## 配置说明

核心配置文件固定在：

```text
.learnBusiness/config/app.toml
```

默认内容类似：

```toml
[ai]
provider = "mock"
base_url = "https://api.openai.com/v1"
chat_model = "gpt-4o-mini"
vision_model = "gpt-4o-mini"
embedding_model = "text-embedding-3-small"
api_key_env = "OPENAI_API_KEY"

[safety]
redact_before_external_ai = true
dry_run_ai = false

[performance]
context_chunks = 5
chunk_char_limit = 1600
```

字段说明：

- `[ai].provider`：AI 提供方名称。当前支持 `mock`、`openai-compatible`、`ollama`、`local-http`。
- `[ai].base_url`：AI 服务基础地址。`ollama` 和 `local-http` 必须使用 `localhost`、`127.0.0.1` 或 `[::1]`，避免误连外部地址。
- `[ai].chat_model`：预留的文本问答模型名。
- `[ai].vision_model`：预留的视觉模型名。
- `[ai].embedding_model`：预留的向量模型名。
- `[ai].api_key_env`：外部 AI 的 API key 环境变量名。配置文件只保存变量名，不保存密钥值。本地 provider 可设为空字符串。
- `[safety].redact_before_external_ai`：接入外部 AI 前的脱敏开关，默认开启。
- `[safety].dry_run_ai`：AI dry-run 默认开关；命令行 `--dry-run-ai` 可用于单次图片检查。
- `[performance].context_chunks`：问答 top-k 内容块数量，默认 5。运行时会读取这个值；当前实现会把有效值限制在 1 到 20 之间，避免一次发送过多上下文。
- `[performance].chunk_char_limit`：导入时单个 chunk 的字符上限，默认 1600。

### 本地 Ollama 启动与配置

先启动 Ollama 并准备模型。模型名可以按本机实际情况替换：

```powershell
ollama serve
ollama pull qwen2.5
ollama pull llava
ollama pull nomic-embed-text
```

工作区配置示例：

```toml
[ai]
provider = "ollama"
base_url = "http://127.0.0.1:11434"
chat_model = "qwen2.5"
vision_model = "llava"
embedding_model = "nomic-embed-text"
api_key_env = ""
```

`base_url` 必须是 loopback 地址，例如 `http://127.0.0.1:11434`。如果写成远程地址，即使是 dry-run 也会被拒绝。

### 通用 local-http 最小协议

`local-http` 用于接入本机自建模型服务，配置示例：

```toml
[ai]
provider = "local-http"
base_url = "http://127.0.0.1:8000/v1"
chat_model = "local-chat"
vision_model = "local-vision"
embedding_model = "local-embedding"
api_key_env = ""
```

服务需要实现三个 JSON endpoint：

- `POST /answer`：请求包含 `purpose`、`model`、`question`、`contexts[{id,text}]`，响应包含 `answer` 和可选 `model`。
- `POST /describe-image`：请求包含 `purpose`、`model`、`prompt`、`image{mime_type,content_hash,base64}`，响应包含 `description` 和可选 `model`。
- `POST /embeddings`：请求包含 `purpose`、`model`、`texts[]`，响应包含 `embeddings[][]` 和可选 `model`。

`local-http` 同样只能使用 loopback 地址，适合把本机 vLLM、llama.cpp server 或自研模型网关包一层轻量协议后接入。

安全建议：

- 不要把 API key 写进 `.learnBusiness/config/app.toml`。
- 本地模型建议使用 `provider = "ollama"` 或 `provider = "local-http"`，并把 `base_url` 限制在 localhost。
- 不要把 `.learnBusiness/` 提交到仓库。
- 修改 `context_chunks` 前先评估 token 成本；数值越大，上下文越多，成本和泄露面也越大。
- 修改 `chunk_char_limit` 前先评估检索质量；过大容易浪费 token，过小可能切碎业务语义。

## 安全注意事项

`learnBusiness` 的默认策略是本地优先，不上传原始文件。导入阶段在本地扫描、hash、提取和建索引；问答阶段只取 top-k chunk；图片 dry-run 不发送图片。

使用时仍需注意：

- 原始资料路径、chunk 文本、AI 调用记录和缓存都可能保存在 `.learnBusiness/` 中，应按敏感数据管理。
- 默认配置文件不保存 API key；真实密钥应放在环境变量或外部密钥管理系统中。
- 当前脱敏规则覆盖邮箱、中国大陆手机号、长数字和 `sk-` 形式密钥，但不能保证识别所有敏感信息。
- 接入外部 AI 前，先用 dry-run 和审计命令确认调用范围。
- 对包含客户信息、合同、财务、人事、源代码密钥的资料，建议先在隔离副本中测试。
- 生成报告和回答是辅助材料，不能替代业务负责人、法务或安全人员的复核。

## 常见问题

### 为什么问答没有来源？

通常是本地索引没有命中。先确认已经执行导入，再把问题改成更贴近文档原文的关键词。

```powershell
cargo run --bin learnBusiness -- ingest .\samples\docs --workspace .\workspace
cargo run --bin learnBusiness -- ask --workspace .\workspace "客户准入规则是什么？"
```

### 为什么重复导入显示 skipped？

这是正常行为。`learnBusiness` 会记录文件内容 hash，未变化文件会跳过，避免重复提取和重复处理。

### 为什么 `.docx` 或 `.pptx` 导入后问不到内容？

当前版本能发现这些文件并判断它们需要 AI 或后续处理，但导入阶段还不会自动抽取其中的正文。请先把关键内容导出为 `.md`、`.txt` 或可抽取文本的 PDF，再导入问答。

### 为什么图片 dry-run 不生成真实图片说明？

`--dry-run-ai` 的目的就是不调用真实 AI，只查看调用计划和记录审计。如果去掉 `--dry-run-ai`，命令会按 `[ai].provider` 调用真实 provider；默认 `mock` 仍只返回确定性的模拟描述。

### Ollama 调用失败怎么办？

先确认服务和模型：

```powershell
ollama list
ollama serve
```

常见原因包括 Ollama 未启动、模型名写错、vision 模型不支持图片、embedding 模型不存在，或 `base_url` 不是 loopback 地址。修复后可重新执行 `ask` 或 `describe-image`。

### OpenAI-compatible 提示缺少 API key 怎么办？

配置文件只能写环境变量名，不能写密钥值。先设置环境变量，再运行命令：

```powershell
$env:OPENAI_API_KEY = "你的密钥"
```

如果公司网关使用其他变量名，把 `[ai].api_key_env` 改成对应名称。

### 如何回退到 mock？

把 `.learnBusiness/config/app.toml` 改回：

```toml
[ai]
provider = "mock"
```

其他模型字段可以保留。回退后不会触发真实 provider HTTP 调用，本地索引、问答来源、报告和审计查看仍可继续使用。

### local-http 返回 JSON 不符合协议怎么办？

`inspect-ai` 会显示失败状态和错误分类。请确认本地服务返回字段名符合最小协议：问答返回 `answer`，图片返回 `description`，embedding 返回 `embeddings`。HTTP 非 2xx 或 JSON 解析失败会被记录为失败类别，不会把完整请求体或业务正文写入审计。

### 能不能直接使用 `learnBusiness` 命令？

可以，但前提是已经构建可执行文件，并把它所在目录加入 `PATH`。开发时最稳妥的写法是：

```powershell
cargo run --bin learnBusiness -- status --workspace .\workspace
```

如果已安装或配置好 PATH，可以写成：

```powershell
learnBusiness status --workspace .\workspace
```

### 配置文件应该放在哪里？

固定放在工作区内的 `.learnBusiness/config/app.toml`。不要改成 `.learnBusiness/config.toml` 或项目根目录的其他配置文件，否则当前工作区初始化和查找逻辑不会按预期工作。

### 如何降低 token 成本？

优先保持默认配置：`context_chunks = 5`、`chunk_char_limit = 1600`。导入时依靠 hash 跳过未变化文件，问答时依靠 top-k 只取相关 chunk。需要更大上下文时可以调高 `context_chunks`，但不要为了“答案更全”盲目提高，应先改进资料结构和提问关键词。

### 如何确认有没有发生 AI 调用？

使用审计命令：

```powershell
cargo run --bin learnBusiness -- inspect-ai --workspace .\workspace
```

如果没有记录，会提示没有 AI 调用记录。图片 dry-run 和非 dry-run 的图片描述都会写入调用记录。

### 可以把 `.learnBusiness/` 发给别人排查吗？

不建议直接发送。里面可能包含本地资料路径、索引文本、缓存和调用记录。排查问题时优先提供命令输出、脱敏后的配置片段和可复现的最小样例。
