# 业务文档理解 Agent 实现计划

> **给 agentic workers：** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**目标：** 构建一个 Rust CLI 纵向切片，支持本地业务文档目录初始化、文件发现、文本/PDF 抽取、SQLite 元数据和全文检索、mock AI、OpenAI-compatible provider 骨架、图片描述入口、问答和基础报告。

**架构：** 第一版采用确定性 pipeline，AI 作为可缓存工具，而不是让模型自由读取全量文档。核心模块围绕 `Workspace`、`Document`、`Chunk`、`MetadataStore`、`Extractor`、`AiProvider`、`QaEngine` 和 `ReportGenerator` 组织。

**技术栈：** Rust 1.95、`clap`、`tokio`、`serde`、`anyhow`、`thiserror`、`rusqlite`、`walkdir`、`sha2`、`uuid`、`chrono`、`regex`、`reqwest`、`pdf-extract`、`tempfile`。

---

## 文件结构

- 创建 `Cargo.toml`：crate 元数据、依赖和 dev-dependencies。
- 创建 `src/main.rs`：CLI 入口和命令分发。
- 创建 `src/lib.rs`：公开内部模块，供集成测试使用。
- 创建 `src/config.rs`：配置模型和 AI provider 配置读取。
- 创建 `src/workspace.rs`：`.agent-index` 工作区初始化和路径管理。
- 创建 `src/models.rs`：`Document`、`Chunk`、`AiCall`、`Citation`、枚举和稳定 ID。
- 创建 `src/discover.rs`：文件发现、类型识别和 SHA-256 hash。
- 创建 `src/store.rs`：SQLite schema、元数据写入、FTS5 全文搜索。
- 创建 `src/ingest/mod.rs`：ingest pipeline。
- 创建 `src/ingest/extract.rs`：txt/md/pdf/image 基础抽取。
- 创建 `src/ai/mod.rs`：`AiProvider` trait、mock provider、OpenAI-compatible provider。
- 创建 `src/ai/cache.rs`：AI cache key 和文件缓存。
- 创建 `src/ai/redaction.rs`：本地脱敏。
- 创建 `src/qa.rs`：混合检索的第一版问答。
- 创建 `src/report.rs`：基础业务理解报告。
- 创建 `src/task.rs`：强类型 `Task`、`Agent`、`Tool` 和权限模型。
- 创建 `tests/cli_flow.rs`：端到端 CLI 流程测试。
- 创建 `tests/fixtures/docs/`：小型文本、图片占位和可选 PDF fixture。

## 执行顺序

任务 1、2、3 必须串行，因为它们建立 crate、基础类型和工作区。任务 4、5、6 可以在基础类型稳定后并行推进。任务 7、8 依赖索引和 AI 抽象。任务 9 做最终 CLI 串联和验收。

---

### 任务 1：初始化 Rust crate 和 CLI 外壳

**文件：**
- 创建：`Cargo.toml`
- 创建：`src/lib.rs`
- 创建：`src/main.rs`
- 测试：`tests/cli_flow.rs`

- [ ] **步骤 1：创建失败的 CLI 测试**

在 `tests/cli_flow.rs` 写入：

```rust
use assert_cmd::Command;

#[test]
fn prints_help_with_core_commands() {
    let mut cmd = Command::cargo_bin("biz-agent").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("ingest"))
        .stdout(predicates::str::contains("ask"))
        .stdout(predicates::str::contains("report"));
}
```

运行：`cargo test prints_help_with_core_commands -- --exact`

预期：失败，原因是还没有 `Cargo.toml` 或 binary。

- [ ] **步骤 2：写最小 crate 配置**

创建 `Cargo.toml`，包名必须是 `biz-agent`，并加入第一批依赖：

```toml
[package]
name = "biz-agent"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1"
base64 = "0.22"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
mime_guess = "2"
pdf-extract = "0.10"
regex = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
rusqlite = { version = "0.32", features = ["bundled", "chrono"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
thiserror = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
uuid = { version = "1", features = ["v5", "serde"] }
walkdir = "2"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

- [ ] **步骤 3：实现 CLI 外壳**

`src/lib.rs`：

```rust
pub mod ai;
pub mod config;
pub mod discover;
pub mod ingest;
pub mod models;
pub mod qa;
pub mod report;
pub mod store;
pub mod task;
pub mod workspace;
```

`src/main.rs` 定义 `init`、`ingest`、`status`、`inspect-ai`、`report`、`ask`、`describe-image` 子命令。每个命令先返回清晰的未接线错误或调用对应模块。

- [ ] **步骤 4：验证 CLI 测试通过**

运行：`cargo test prints_help_with_core_commands -- --exact`

预期：通过，stdout 包含 `ingest`、`ask`、`report`。

- [ ] **步骤 5：提交**

```bash
git add Cargo.toml src/lib.rs src/main.rs tests/cli_flow.rs
git commit -m "feat: add Rust CLI shell"
```

---

### 任务 2：模型、权限和任务抽象

**文件：**
- 创建：`src/models.rs`
- 创建：`src/task.rs`
- 修改：`src/lib.rs`

- [ ] **步骤 1：写失败测试**

在 `src/models.rs` 的 `#[cfg(test)]` 模块中测试：

```rust
#[test]
fn chunk_id_is_stable_for_same_source() {
    let first = Chunk::stable_id("doc-1", ChunkKind::Text, Some(3), None, "hello");
    let second = Chunk::stable_id("doc-1", ChunkKind::Text, Some(3), None, "hello");
    assert_eq!(first, second);
}
```

在 `src/task.rs` 的 `#[cfg(test)]` 模块中测试：

```rust
#[test]
fn tool_permission_denies_ungranted_external_ai() {
    let tool = ToolDescriptor::new("describe_image", Permission::AiExternal);
    let grants = PermissionSet::new(vec![Permission::ReadLocal]);
    assert!(tool.ensure_allowed(&grants).is_err());
}
```

运行：`cargo test chunk_id_is_stable_for_same_source tool_permission_denies_ungranted_external_ai`

预期：失败，类型还不存在。

- [ ] **步骤 2：实现最小模型**

实现 `Document`、`Chunk`、`ChunkKind`、`AiCall`、`Citation`。`Chunk::stable_id` 使用 UUID v5，命名空间固定为 `Uuid::NAMESPACE_URL`，输入包含 document id、kind、page、slide 和 content hash。

- [ ] **步骤 3：实现权限模型**

实现 `Permission`、`PermissionSet`、`ToolDescriptor`、`AgentDescriptor`、`TaskDescriptor`。`ToolDescriptor::ensure_allowed` 在权限缺失时返回错误。

- [ ] **步骤 4：验证测试通过**

运行：`cargo test chunk_id_is_stable_for_same_source tool_permission_denies_ungranted_external_ai`

预期：两个测试通过。

- [ ] **步骤 5：提交**

```bash
git add src/models.rs src/task.rs src/lib.rs
git commit -m "feat: add core models and task permissions"
```

---

### 任务 3：工作区初始化和配置

**文件：**
- 创建：`src/workspace.rs`
- 创建：`src/config.rs`
- 修改：`src/main.rs`
- 测试：`tests/cli_flow.rs`

- [ ] **步骤 1：写失败测试**

在 `tests/cli_flow.rs` 添加：

```rust
#[test]
fn init_creates_agent_index_layout() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("biz-agent")
        .unwrap()
        .arg("init")
        .arg(temp.path())
        .assert()
        .success();

    assert!(temp.path().join(".agent-index/config.toml").exists());
    assert!(temp.path().join(".agent-index/artifacts/images").exists());
    assert!(temp.path().join(".agent-index/cache/ai").exists());
}
```

运行：`cargo test init_creates_agent_index_layout -- --exact`

预期：失败，`init` 还没有创建目录。

- [ ] **步骤 2：实现 `Workspace`**

`Workspace::init(root)` 创建 `.agent-index`、`artifacts/images`、`artifacts/pages`、`artifacts/thumbnails`、`cache/ai`、`cache/extraction`、`logs`，并写入默认 `config.toml`。

- [ ] **步骤 3：接线 `biz-agent init`**

`main.rs` 的 `Commands::Init { workspace }` 调用 `Workspace::init`，成功后打印工作区路径。

- [ ] **步骤 4：验证测试通过**

运行：`cargo test init_creates_agent_index_layout -- --exact`

预期：通过。

- [ ] **步骤 5：提交**

```bash
git add src/workspace.rs src/config.rs src/main.rs tests/cli_flow.rs
git commit -m "feat: initialize local workspace"
```

---

### 任务 4：文件发现、hash 和基础抽取

**文件：**
- 创建：`src/discover.rs`
- 创建：`src/ingest/mod.rs`
- 创建：`src/ingest/extract.rs`
- 修改：`src/lib.rs`

- [ ] **步骤 1：写失败测试**

在 `src/discover.rs` 测试：

```rust
#[test]
fn discovers_supported_documents_with_hashes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
    std::fs::write(dir.path().join("b.exe"), "skip").unwrap();

    let docs = discover_documents(dir.path()).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].file_type, "text/plain");
    assert_eq!(docs[0].sha256.len(), 64);
}
```

在 `src/ingest/extract.rs` 测试：

```rust
#[test]
fn extracts_plain_text_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sample.txt");
    std::fs::write(&file, "业务流程").unwrap();

    let extracted = extract_document_text(&file, "text/plain").unwrap();
    assert!(extracted.text.contains("业务流程"));
}
```

运行：`cargo test discovers_supported_documents_with_hashes extracts_plain_text_file`

预期：失败，函数还不存在。

- [ ] **步骤 2：实现发现逻辑**

支持扩展名：`txt`、`md`、`pdf`、`png`、`jpg`、`jpeg`、`webp`、`docx`、`pptx`。第一版对 `docx` 和 `pptx` 只登记元数据，不抽取正文。

- [ ] **步骤 3：实现抽取逻辑**

`txt` 和 `md` 使用 UTF-8 读取。`pdf` 使用 `pdf_extract::extract_text(path)`。图片返回空文本和 artifact 标记。

- [ ] **步骤 4：验证测试通过**

运行：`cargo test discovers_supported_documents_with_hashes extracts_plain_text_file`

预期：通过。

- [ ] **步骤 5：提交**

```bash
git add src/discover.rs src/ingest/mod.rs src/ingest/extract.rs src/lib.rs
git commit -m "feat: discover and extract local documents"
```

---

### 任务 5：SQLite 元数据和全文索引

**文件：**
- 创建：`src/store.rs`
- 修改：`src/ingest/mod.rs`

- [ ] **步骤 1：写失败测试**

在 `src/store.rs` 添加：

```rust
#[test]
fn stores_chunks_and_searches_full_text() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("metadata.sqlite");
    let store = MetadataStore::open(&db).unwrap();

    let doc = DocumentRecord::new_for_test("doc-1", "sample.txt", "text/plain");
    store.upsert_document(&doc).unwrap();
    store.insert_chunk("chunk-1", "doc-1", "text", "客户准入规则", None, None).unwrap();

    let results = store.search_text("准入", 5).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].chunk_id, "chunk-1");
}
```

运行：`cargo test stores_chunks_and_searches_full_text -- --exact`

预期：失败，store 还不存在。

- [ ] **步骤 2：实现 schema**

创建表：`documents`、`chunks`、`ai_calls`，并创建 FTS5 虚拟表 `chunks_fts`。插入 chunk 时同步写入 FTS。

- [ ] **步骤 3：实现查询**

`search_text(query, limit)` 返回 chunk id、document path、text snippet、score 占位值。

- [ ] **步骤 4：验证测试通过**

运行：`cargo test stores_chunks_and_searches_full_text -- --exact`

预期：通过。

- [ ] **步骤 5：提交**

```bash
git add src/store.rs src/ingest/mod.rs
git commit -m "feat: store metadata and full text index"
```

---

### 任务 6：AI provider、缓存和脱敏

**文件：**
- 创建：`src/ai/mod.rs`
- 创建：`src/ai/cache.rs`
- 创建：`src/ai/redaction.rs`
- 修改：`src/lib.rs`

- [ ] **步骤 1：写失败测试**

在 `src/ai/cache.rs` 测试：

```rust
#[test]
fn ai_cache_key_changes_when_prompt_version_changes() {
    let a = AiCacheKey::new("openai", "gpt-4o-mini", "describe_image", "v1", "abc", true);
    let b = AiCacheKey::new("openai", "gpt-4o-mini", "describe_image", "v2", "abc", true);
    assert_ne!(a.to_filename(), b.to_filename());
}
```

在 `src/ai/redaction.rs` 测试：

```rust
#[test]
fn redacts_email_and_phone() {
    let input = "联系人 test@example.com 电话 13800138000";
    let output = redact_sensitive_text(input);
    assert!(!output.contains("test@example.com"));
    assert!(!output.contains("13800138000"));
}
```

运行：`cargo test ai_cache_key_changes_when_prompt_version_changes redacts_email_and_phone`

预期：失败，模块还不存在。

- [ ] **步骤 2：实现 `AiProvider` trait 和 mock provider**

`AiProvider` 暴露 `describe_image`、`summarize_chunks`、`embed_texts`、`answer`。`MockAiProvider` 返回确定性文本，便于测试。

- [ ] **步骤 3：实现 OpenAI-compatible provider 骨架**

`OpenAiCompatibleProvider` 读取 `base_url`、`api_key`、`chat_model`、`vision_model`、`embedding_model`。没有 API key 时返回清晰错误，不在测试里真实联网。

- [ ] **步骤 4：实现 cache 和 redaction**

cache 文件名使用 SHA-256。脱敏覆盖邮箱、中国大陆手机号、长数字和 `sk-` 开头的密钥样式字符串。

- [ ] **步骤 5：验证测试通过**

运行：`cargo test ai_cache_key_changes_when_prompt_version_changes redacts_email_and_phone`

预期：通过。

- [ ] **步骤 6：提交**

```bash
git add src/ai/mod.rs src/ai/cache.rs src/ai/redaction.rs src/lib.rs
git commit -m "feat: add AI provider cache and redaction"
```

---

### 任务 7：ingest 命令串联

**文件：**
- 修改：`src/ingest/mod.rs`
- 修改：`src/main.rs`
- 修改：`tests/cli_flow.rs`

- [ ] **步骤 1：写失败测试**

在 `tests/cli_flow.rs` 添加：

```rust
#[test]
fn ingest_indexes_text_document() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(docs.path().join("policy.txt"), "客户准入规则").unwrap();

    Command::cargo_bin("biz-agent").unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("biz-agent").unwrap()
        .args([
            "ingest",
            docs.path().to_str().unwrap(),
            "--workspace",
            workspace.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(workspace.path().join(".agent-index/metadata.sqlite").exists());
}
```

运行：`cargo test ingest_indexes_text_document -- --exact`

预期：失败，`ingest` 还没有接入 store。

- [ ] **步骤 2：实现 pipeline**

`run_ingest(workspace, docs_dir)` 执行发现、抽取、document upsert、chunk insert。文件 hash 未变化时跳过 chunk 重建。

- [ ] **步骤 3：接线 CLI**

`biz-agent ingest <docs_dir> --workspace <workspace>` 调用 pipeline，并输出 scanned、indexed、skipped、warnings 计数。

- [ ] **步骤 4：验证测试通过**

运行：`cargo test ingest_indexes_text_document -- --exact`

预期：通过。

- [ ] **步骤 5：提交**

```bash
git add src/ingest/mod.rs src/main.rs tests/cli_flow.rs
git commit -m "feat: ingest documents into local index"
```

---

### 任务 8：问答、图片描述和报告

**文件：**
- 创建：`src/qa.rs`
- 创建：`src/report.rs`
- 修改：`src/main.rs`
- 修改：`tests/cli_flow.rs`

- [ ] **步骤 1：写失败测试**

在 `tests/cli_flow.rs` 添加：

```rust
#[test]
fn ask_returns_answer_with_source() {
    let workspace = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    std::fs::write(docs.path().join("process.txt"), "核心流程是申请、审核、归档。").unwrap();

    Command::cargo_bin("biz-agent").unwrap()
        .args(["init", workspace.path().to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("biz-agent").unwrap()
        .args(["ingest", docs.path().to_str().unwrap(), "--workspace", workspace.path().to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("biz-agent").unwrap()
        .args(["ask", "--workspace", workspace.path().to_str().unwrap(), "核心流程是什么？"])
        .assert()
        .success()
        .stdout(predicates::str::contains("process.txt"));
}
```

运行：`cargo test ask_returns_answer_with_source -- --exact`

预期：失败，`ask` 未实现。

- [ ] **步骤 2：实现 `QaEngine`**

先用全文检索取 top-k chunk，交给 `MockAiProvider::answer` 生成确定性答案，输出引用来源。

- [ ] **步骤 3：实现 `ReportGenerator`**

`report --workspace <workspace> --out report.md` 生成包含“执行摘要”“资料集概览”“核心业务对象”“主要业务流程”“需要确认的问题”“来源引用”的 Markdown。

- [ ] **步骤 4：实现 `describe-image`**

`describe-image <image_path> --workspace <workspace> --dry-run-ai` 输出将要发送的图片 hash 和用途；不加 dry-run 时用 provider 描述图片并写入 AI cache。

- [ ] **步骤 5：验证测试通过**

运行：`cargo test ask_returns_answer_with_source -- --exact`

预期：通过。

- [ ] **步骤 6：提交**

```bash
git add src/qa.rs src/report.rs src/main.rs tests/cli_flow.rs
git commit -m "feat: answer questions and generate reports"
```

---

### 任务 9：最终验收和清理

**文件：**
- 修改：`README.md`
- 修改：`Cargo.toml`
- 修改：`Cargo.lock`
- 修改：`src/main.rs`
- 修改：`src/lib.rs`
- 修改：`src/config.rs`
- 修改：`src/workspace.rs`
- 修改：`src/models.rs`
- 修改：`src/discover.rs`
- 修改：`src/store.rs`
- 修改：`src/ingest/mod.rs`
- 修改：`src/ingest/extract.rs`
- 修改：`src/ai/mod.rs`
- 修改：`src/ai/cache.rs`
- 修改：`src/ai/redaction.rs`
- 修改：`src/qa.rs`
- 修改：`src/report.rs`
- 修改：`src/task.rs`
- 修改：`tests/cli_flow.rs`

- [ ] **步骤 1：写 README**

`README.md` 必须包含中文快速开始：

````markdown
# biz-agent

本项目是一个本地优先的业务文档理解 agent。第一版支持初始化工作区、ingest 文本和基础 PDF、建立 SQLite/全文索引、基于索引问答、生成基础报告，并预留多模态 AI provider。

## 快速开始

```powershell
cargo run -- init .\workspace
cargo run -- ingest .\docs --workspace .\workspace
cargo run -- ask --workspace .\workspace "这个业务的核心流程是什么？"
cargo run -- report --workspace .\workspace --out report.md
```
````

- [ ] **步骤 2：运行完整验证**

运行：

```powershell
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

预期：三个命令全部 exit 0。

- [ ] **步骤 3：修复验证发现的问题**

只修复 `fmt`、`clippy`、`test` 明确指出的问题，不做无关重构。

- [ ] **步骤 4：最终提交**

```bash
git add README.md Cargo.toml Cargo.lock src tests
git commit -m "docs: add usage guide for business document agent"
```

---

## 自检清单

- 设计目标覆盖：CLI、工作区、发现、抽取、元数据、全文检索、AI provider、缓存、脱敏、问答、报告和图片描述入口都有对应任务。
- 第一版边界清晰：DOCX/PPTX 在本切片登记元数据，不做深度抽取；MCP 只保留抽象，不接真实 server。
- TDD 顺序明确：每个功能任务先写失败测试，再实现，再验证，再提交。
- 并行边界明确：任务 4、5、6 可以在任务 2 和 3 完成后并行；任务 7、8、9 串联收口。
