# learnBusiness 操作手册

## 用途

`learnBusiness` 是一个本地优先、轻量、省 token 的业务文档理解工具。它适合把一批业务资料放进本地工作区，先建立索引，再围绕资料做问答、生成基础报告，并在需要处理图片时通过多模态 AI 接口生成描述。

默认 AI provider 是 `mock`，适合离线验证工作流、索引、报告和调用审计。需要真实模型时，在 `.learnBusiness/config/app.toml` 中切换到 `provider = "http"`。`http` 表示可配置 HTTP AI 接口，`base_url` 可以是 `localhost`、企业网关或云端接口，不代表必须本地部署大模型。

## 环境准备

```powershell
rustc --version
cargo --version
```

进入项目目录：

```powershell
Set-Location -LiteralPath "C:\projects\learnBusiness"
```

准备样例资料目录：

```powershell
New-Item -ItemType Directory -Force .\samples\docs
Set-Content -Encoding UTF8 .\samples\docs\process.md "# 业务流程`n客户提交申请，运营审核，系统归档。"
```

## 构建

开发环境推荐直接用 Cargo 运行：

```powershell
cargo run --bin learnBusiness -- --help
```

构建可执行文件：

```powershell
cargo build --bin learnBusiness
```

构建完成后可执行文件通常位于：

```powershell
.\target\debug\learnBusiness.exe
```

## 初始化工作区

```powershell
cargo run --bin learnBusiness -- init .\workspace
```

初始化后会生成：

```text
workspace\
  .learnBusiness\
    config\
      app.toml
    metadata.sqlite
    fulltext\
    vectors\
    artifacts\
    cache\
    logs\
```

查看配置：

```powershell
Get-Content -LiteralPath .\workspace\.learnBusiness\config\app.toml
```

`.learnBusiness/` 是本地运行状态目录，通常不应提交到仓库。

## 导入文档

```powershell
cargo run --bin learnBusiness -- ingest .\samples\docs --workspace .\workspace
```

显式处理图片描述并写入索引：

```powershell
cargo run --bin learnBusiness -- ingest .\samples\docs --workspace .\workspace --describe-images
```

只预览图片 AI 调用计划，不写图片描述 chunk：

```powershell
cargo run --bin learnBusiness -- ingest .\samples\docs --workspace .\workspace --describe-images --dry-run-ai
```

当前行为：

- `.txt` 和 `.md` 按文本读取。
- `.pdf` 尝试抽取基础文本。
- `.docx` 会从 Office Open XML 的 `word/document.xml` 抽取正文段落。
- `.pptx` 会从 `ppt/slides/slide*.xml` 抽取幻灯片文本，并保留 slide 编号。
- 图片会被发现并登记为需要 AI/OCR 后续处理的资料；只有显式传入 `--describe-images` 才会调用多模态 AI 并把描述写入索引。
- 不支持的扩展名会忽略。
- 未变化文件按内容 hash 跳过。

省 token 机制：

- 文件内容 hash 避免重复导入。
- 文本默认按约 1600 字符切成 chunk。
- 问答默认只取最相关的 5 个 chunk，可通过 `performance.context_chunks` 配置。

查看工作区状态：

```powershell
cargo run --bin learnBusiness -- status --workspace .\workspace
```

## 本地检索

```powershell
cargo run --bin learnBusiness -- search --workspace .\workspace --limit 5 "运营审核"
```

`search` 只查询本地 FTS 索引，不调用 AI provider，不写 AI 审计。输出包括：

- 文件路径
- `chunk`
- `kind`
- `score`
- `ai_generated`
- `page` 或 `slide`
- `artifact`
- `snippet`

建议先用 `search` 检查资料是否命中，再执行 `ask`。

## 问答

```powershell
cargo run --bin learnBusiness -- ask --workspace .\workspace "这个业务的核心流程是什么？"
```

问答会先在本地 FTS 索引里检索相关 chunk，再把少量上下文交给当前 AI provider。没有检索命中时不会调用 AI。

命中来源会输出可定位引用，例如：

```text
来源:
- docs\process.pptx chunk=... score=-1.2345 slide=2
```

`score` 来自 SQLite FTS5 排序，数值用于判断同一次检索中的相对相关性；`slide` 或 `page` 存在时可用于回到原资料核对。

建议：

- 使用资料中的关键词提问。
- 问题太泛时，改成更贴近文档原文的表达。
- 先导入资料，再问答。

## 生成报告

```powershell
cargo run --bin learnBusiness -- report --workspace .\workspace --out .\workspace\report.md
```

查看报告：

```powershell
Get-Content -LiteralPath .\workspace\report.md
```

当前报告是本地规则提取的候选线索，包含业务对象、流程候选、规则/约束、风险/待确认和来源引用，不替代业务负责人复核。

## 图片 Dry-run

```powershell
cargo run --bin learnBusiness -- describe-image .\samples\docs\flow.png --workspace .\workspace --dry-run-ai
```

dry-run 会计算图片 hash、识别 MIME 类型、写入一条 AI 调用审计，但不发送图片、不执行真实 HTTP 请求。

输出字段包括：

- `purpose`
- `provider`
- `model`
- `image`
- `input_hash` / `sha256`
- `mime`
- `redaction`
- `token_estimate`
- `local_provider`
- `trace_id` 可通过 `inspect-ai` 查看

当前字段名 `local_provider` 表示配置的 `base_url` 是否为 loopback HTTP 端点，不表示一定使用本地模型。

## 查看 AI 调用审计

```powershell
cargo run --bin learnBusiness -- inspect-ai --workspace .\workspace
```

输出字段包括：

- `purpose`
- `provider`
- `model`
- `status`
- `trace_id`
- `input_hash`
- `output_hash`
- `redaction`
- `token_estimate`
- `error_category`

建议在接入真实 HTTP AI 前，先用 `describe-image --dry-run-ai` 和 `inspect-ai` 验证审计链路。

按 trace id 过滤：

```powershell
cargo run --bin learnBusiness -- inspect-ai --workspace .\workspace --trace <trace_id>
```

## 配置说明

配置文件固定为：

```text
.learnBusiness/config/app.toml
```

默认配置类似：

```toml
[ai]
provider = "mock"
base_url = "http://localhost:8000/v1"
chat_model = "business-chat"
vision_model = "business-vision"
embedding_model = "business-embedding"
api_key_env = ""

[ai.headers]
# Authorization = "Bearer ${LEARNBUSINESS_AI_KEY}"

[safety]
redact_before_external_ai = true
dry_run_ai = false

[performance]
context_chunks = 5
chunk_char_limit = 1600

[logging]
trace_enabled = true
```

字段说明：

- `[ai].provider`：`mock` 或 `http`。`openai-compatible` 作为旧配置别名仍兼容，但新配置应使用 `http`。
- `[ai].base_url`：HTTP AI 服务基础地址。可以是 `http://localhost:8000/v1`，也可以是 `https://gateway.example.com/v1`。
- `[ai].chat_model`：问答和摘要模型名。
- `[ai].vision_model`：多模态图片理解模型名。
- `[ai].embedding_model`：embedding 模型名。
- `[ai].api_key_env`：兼容快捷方式。未显式配置 `Authorization` 请求头时，会用该环境变量生成 bearer token。
- `[ai.headers]`：发送给 HTTP AI 服务的请求头。值支持 `${ENV_NAME}` 占位符。
- `[safety].redact_before_external_ai`：远程 HTTP AI 调用前是否脱敏。
- `[performance].context_chunks`：问答 top-k chunk 数量，运行时限制在 1 到 20。
- `[performance].chunk_char_limit`：发送给 provider 前的单 chunk 字符上限。
- `[logging].trace_enabled`：是否写入 `.learnBusiness/logs/trace.jsonl`。

## 通用 HTTP Provider

配置示例：

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

设置环境变量：

```powershell
$env:LEARNBUSINESS_AI_KEY = "你的密钥"
```

当前 HTTP provider 使用以下路径：

- `POST {base_url}/chat/completions`：问答、摘要、图片理解。
- `POST {base_url}/embeddings`：embedding。

多模态图片请求会把图片编码为 data URL，并使用同一套 `[ai.headers]`。请求头缺失环境变量会在发起网络请求前失败，并被记录为 AI 调用失败。

## 安全建议

- 不要把真实 token 写入 `.learnBusiness/config/app.toml`。
- 不要提交 `.learnBusiness/`。
- 接入远程 HTTP AI 前保持 `redact_before_external_ai = true`。
- 不要盲目调高 `context_chunks`；数值越大，成本和泄漏面越大。
- 如果网关需要额外 header，只添加必要字段，且优先使用环境变量占位符。

## 常见问题

### 问答没有来源怎么办？

通常是本地索引没有命中。先确认已执行导入，再把问题改成更贴近文档原文的关键词。

```powershell
cargo run --bin learnBusiness -- ingest .\samples\docs --workspace .\workspace
cargo run --bin learnBusiness -- ask --workspace .\workspace "客户准入规则是什么？"
```

### `.docx` 或 `.pptx` 导入后问不到内容怎么办？

当前版本会抽取 Office Open XML 中的正文段落和幻灯片文本，但复杂版面、嵌入图片、SmartArt、备注和扫描件仍可能抽不到。可先把关键内容导出为 `.md`、`.txt` 或可抽取文本的 PDF；后续可接 OCR 或多模态 AI 补全。

### 图片 dry-run 为什么不生成真实说明？

`--dry-run-ai` 的目的就是不调用真实 AI，只查看调用计划和审计记录。去掉该参数后会按 `[ai].provider` 调用 provider；默认 `mock` 仍只返回确定性的模拟描述。

### HTTP 调用失败怎么排查？

先看审计：

```powershell
cargo run --bin learnBusiness -- inspect-ai --workspace .\workspace
```

再看 trace：

```powershell
Get-Content -LiteralPath .\workspace\.learnBusiness\logs\trace.jsonl
```

常见原因：

- `base_url` 写错或服务不可达。
- `${ENV_NAME}` 对应环境变量未设置。
- 网关要求的 header 缺失。
- 响应不是兼容 JSON 格式。
- 模型名不被服务端支持。

### 缺少请求头环境变量怎么办？

在当前 PowerShell 会话里设置：

```powershell
$env:LEARNBUSINESS_AI_KEY = "你的密钥"
```

或者把 `[ai.headers]` 改成网关实际需要的变量名：

```toml
[ai.headers]
Authorization = "Bearer ${COMPANY_AI_TOKEN}"
X-Tenant = "${COMPANY_AI_TENANT}"
```

### 如何回退到 mock？

修改 `.learnBusiness/config/app.toml`：

```toml
[ai]
provider = "mock"
```

其他模型字段可以保留。回退后不会触发真实 HTTP 调用。

### 如何降低 token 成本？

优先保持默认配置：

```toml
[performance]
context_chunks = 5
chunk_char_limit = 1600
```

需要更多上下文时可以调高 `context_chunks`，但不要为了“答案更全”盲目增加。更有效的做法通常是改进资料结构和提问关键词。

### search 和 ask 怎么配合？

先用 `search` 看本地索引是否命中关键资料；确认命中后再用 `ask`。这样可以避免没有来源时无效调用 AI，也更容易定位资料缺口。
