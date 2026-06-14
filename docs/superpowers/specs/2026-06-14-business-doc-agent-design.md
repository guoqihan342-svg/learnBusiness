# Business Document Understanding Agent Design

## Context

The agent is for understanding business documents, not source code. The input is a local folder of business material that may include PDF, Word, PowerPoint, images, scanned pages, diagrams, tables, and mixed office exports.

The system must be token-efficient, safe by default, fast enough for repeated local use, and extensible through AI providers, skills, and MCP servers. Rust is the implementation language.

We reviewed `mainframecomputer/orchestra` as a reference. Its useful ideas are the `Task / Agent / Tool` abstractions, task-level orchestration, explicit task dependencies, MCP adapter shape, iteration limits, duplicate tool-call detection, model fallbacks, and event callbacks. Its document-processing coverage is limited, so this design uses Orchestra only as an orchestration reference, not as the document ingestion model.

Reference:

- https://github.com/mainframecomputer/orchestra
- https://github.com/mainframecomputer/orchestra/blob/main/packages/python/src/mainframe_orchestra/task.py
- https://github.com/mainframecomputer/orchestra/blob/main/packages/python/src/mainframe_orchestra/orchestration.py
- https://github.com/mainframecomputer/orchestra/blob/main/packages/python/src/mainframe_orchestra/adapters/mcp_adapter.py

## Goals

- Ingest a local folder of business documents.
- Extract text, tables, images, slides, pages, and AI-generated image descriptions into traceable chunks.
- Build a local index that supports keyword search, semantic search, citations, reporting, and question answering.
- Use AI only when it adds value: image understanding, scanned-page understanding, complex table interpretation, summarization, report writing, and RAG answers.
- Cache AI calls by stable content hash so repeated runs are cheap.
- Support OpenAI-compatible multimodal and embedding APIs behind a provider trait.
- Prepare extension points for MCP tools and business-domain skills.
- Keep raw files local unless a configured AI or MCP call explicitly needs selected content.

## Non-Goals For First Version

- No full autonomous multi-agent free-form planning loop.
- No mandatory external vector database.
- No complete business knowledge graph.
- No GUI.
- No cloud document sync unless provided later through an MCP server.
- No editing original business documents.

## User-Facing Workflow

```text
biz-agent init <workspace>
biz-agent ingest <docs_dir> --workspace <workspace>
biz-agent status --workspace <workspace>
biz-agent inspect-ai --workspace <workspace>
biz-agent report --workspace <workspace> --out report.md
biz-agent ask --workspace <workspace> "这个业务的核心流程是什么？"
```

`init` creates the local workspace. `ingest` parses documents and builds indexes. `inspect-ai` shows queued or completed AI calls with reasons, hashes, estimated tokens, and redaction status. `report` generates a business understanding report from the index. `ask` answers with citations back to files, pages, slides, or extracted images.

## High-Level Architecture

```text
Business document folder
  -> File discovery and hash scan
  -> Format-specific extraction
  -> Normalized content chunks
  -> Local metadata, full-text, and vector indexes
  -> Selective AI enrichment
  -> Report and RAG question answering
```

Rust owns orchestration, parsing, caching, indexing, safety checks, and CLI execution. AI providers are tools, not the source of truth. The source of truth is the local normalized chunk store plus indexes.

## Workspace Layout

```text
.agent-index/
  config.toml
  metadata.sqlite
  fulltext/
  vectors/
  artifacts/
    images/
    pages/
    thumbnails/
  cache/
    ai/
    extraction/
  logs/
    ingest.jsonl
    ai.jsonl
```

`metadata.sqlite` stores documents, chunks, tasks, AI call records, and citations. `fulltext` stores text-search state. `vectors` stores embeddings and vector metadata. `artifacts` stores extracted page images, embedded document images, and rendered slide/page snapshots when needed.

## Data Model

```text
Document
  id
  path
  file_type
  content_hash
  modified_at
  size_bytes
  ingest_status

Chunk
  id
  document_id
  kind: text | table | image | page | slide | ai_summary | ocr_text
  text
  page
  slide
  source_range
  artifact_path
  confidence
  ai_generated
  content_hash

AiCall
  id
  task_id
  provider
  model
  purpose
  input_hash
  output_hash
  token_estimate
  redaction_applied
  status

Citation
  chunk_id
  document_path
  page
  slide
  source_range
```

Every generated report section or answer should be able to cite one or more `Chunk` records. AI-generated chunks are marked explicitly so downstream answers can distinguish source text from interpretation.

## Indexing Strategy

The first version builds three indexes:

- Metadata index in SQLite for documents, chunks, hashes, task state, and citations.
- Full-text index for exact search over extracted text, OCR text, table text, and AI image descriptions.
- Vector index for semantic retrieval over text chunks, image descriptions, page summaries, slide summaries, and document summaries.

The retrieval path for `ask` is hybrid:

```text
question
  -> keyword search candidates
  -> vector search candidates
  -> metadata filters and dedupe
  -> top-k context pack
  -> AI answer with citations
```

This avoids relying only on vector search and keeps answers traceable.

## Document Extraction

PDF handling:

- Extract embedded text when available.
- Render selected pages to images when page text is missing, low confidence, or contains important diagrams.
- Extract images when feasible and attach them to page-level chunks.

Word handling:

- Extract paragraphs, headings, tables, and embedded images.
- Preserve document order and section hierarchy where available.

PowerPoint handling:

- Extract slide text, speaker notes if available, tables, and embedded images.
- Render slide snapshots for multimodal analysis when the slide has diagrams, charts, or sparse text.

Image handling:

- Store the original image artifact.
- Generate an AI image description only when the image is referenced by a document chunk, likely contains business meaning, or is explicitly requested.

The parser layer may use Rust libraries first and external converters where needed. External tools must be capability-checked at startup and reported clearly in `status`.

## AI Strategy

AI calls are selective and cached.

Use AI for:

- Describing diagrams, process flows, architecture diagrams, org charts, screenshots, scanned pages, and complex tables.
- Summarizing chunk groups into page, document, and corpus summaries.
- Generating the final business understanding report.
- Answering questions from retrieved context.

Avoid AI for:

- Text extraction that can be done locally.
- Reprocessing unchanged files.
- Sending whole documents when only selected chunks are needed.

Provider interface:

```text
AiProvider
  describe_image(image, prompt) -> ImageUnderstanding
  summarize_chunks(chunks, prompt) -> Summary
  embed_texts(texts) -> Embeddings
  answer(question, contexts) -> Answer
```

The first provider should target OpenAI-compatible HTTP APIs with configurable `base_url`, `api_key`, model names, timeouts, and retry policy. The interface should allow later Azure OpenAI, local model gateways, and enterprise AI gateways.

## Token Efficiency

- Stable file and chunk hashes skip unchanged work.
- AI cache key includes provider, model, purpose, prompt version, content hash, and redaction mode.
- Ingest uses local parsing before AI enrichment.
- Question answering sends only top-k retrieved chunks.
- Report generation uses layered summaries rather than raw corpus stuffing.
- Tool loops have explicit iteration limits.
- Large AI outputs use structured JSON schemas to reduce retries and parsing ambiguity.

## Safety

The default behavior is local-first.

- Raw files stay local unless a selected AI or MCP tool call requires content.
- `--dry-run-ai` shows what would be sent, why, and estimated size without making calls.
- Local redaction masks obvious phone numbers, emails, identity numbers, bank-card-like numbers, and secret-like strings before external AI calls when enabled.
- AI logs store hashes, model names, purpose, token estimates, status, and redaction status. They do not store full sensitive prompts by default.
- MCP tools are disabled unless configured.
- Each tool has an explicit permission class: `read_local`, `write_workspace`, `external_network`, `ai_external`, or `mcp_external`.
- The CLI should fail closed when a tool requests a permission not granted by config.

## Orchestration Model

The design borrows Orchestra's task-centric model but makes it strongly typed and deterministic by default.

```text
Task
  id
  kind
  input_refs
  output_refs
  required_permissions
  token_budget
  max_iterations
  status

Agent
  id
  role
  goal
  allowed_tools
  model_policy

Tool
  name
  schema
  permission
  implementation: local | ai | mcp
```

First-version agents:

- `ingest_agent`: document discovery, extraction, chunking, and artifact generation.
- `vision_agent`: image, diagram, scanned-page, and slide-snapshot understanding.
- `index_agent`: metadata, full-text, and vector index updates.
- `report_agent`: business understanding report generation.
- `qa_agent`: retrieval-augmented question answering.

These agents are not free-roaming personas in the first version. They are typed execution units with bounded tools and explicit task inputs.

## MCP And Skill Extension Points

MCP support should follow the adapter shape seen in Orchestra:

- Connect to stdio or SSE MCP servers.
- List available tools.
- Convert tool schemas into local typed tool descriptors.
- Apply permission checks before any call.
- Add optional metadata to tool calls, such as workspace id, tenant id, or trace id.
- Close sessions and child processes predictably.

Skills are domain templates, not executable trust boundaries. A skill can provide prompts, report sections, entity definitions, and domain question checklists such as "insurance business", "supply chain", or "financial risk". The engine still controls parsing, indexing, permissions, and AI calls.

## Business Report Shape

The first report should include:

- Executive summary.
- Document corpus overview.
- Business domain and scope.
- Key roles and stakeholders.
- Core business objects.
- Main business processes.
- Business rules and constraints.
- Systems, forms, data, and integration points mentioned in the documents.
- Risks, ambiguities, and contradictions.
- Open questions for domain experts.
- Source citations.

The report should cite source chunks and clearly label AI-inferred statements.

## Error Handling

- A single file failure records a warning and does not stop the whole ingest.
- AI failures leave local extraction results intact and mark affected chunks as `needs_ai`.
- Missing external converters are reported by `status` with install guidance.
- Corrupt or password-protected files are skipped with clear diagnostics.
- Invalid AI JSON responses are retried within the configured retry limit.
- Repeated identical tool calls are stopped and recorded.
- Max iteration and token budget exhaustion produce partial results instead of silent failure.

## Testing Strategy

Unit tests:

- File hashing.
- Chunk id stability.
- Redaction.
- AI cache key generation.
- JSON schema parsing.
- Permission checks.
- Retrieval dedupe.

Integration tests:

- Small folder with PDF, DOCX, PPTX, PNG, and mixed extracted artifacts.
- Ingest without AI.
- Ingest with mock AI provider.
- Re-ingest unchanged files and verify cache hits.
- Ask a question and verify cited chunks.
- Generate report and verify required sections.

Performance checks:

- Many small files.
- One large PDF.
- PPTX with many images.
- Repeated ingest after minor changes.

## Implementation Notes

Rust crate layout should stay modular:

```text
src/
  main.rs
  config.rs
  workspace.rs
  discover.rs
  task.rs
  agent.rs
  tool.rs
  ingest/
  index/
  ai/
  mcp/
  report.rs
  qa.rs
```

Recommended crate choices should be validated during implementation rather than locked in this design. Likely candidates include `clap`, `tokio`, `serde`, `reqwest`, `sqlx` or `rusqlite`, `tantivy`, and a small local vector store abstraction.

## First Implementation Slice

The first implementation plan should build a vertical slice:

1. CLI and workspace initialization.
2. File discovery, hashing, and metadata SQLite.
3. Plain text and minimal PDF text extraction.
4. Chunk store and full-text search.
5. Mock AI provider and AI cache.
6. `ask` over local search with mock answer.
7. OpenAI-compatible provider behind config.
8. Image description path for standalone images.
9. Basic report generation.

This keeps the first version testable before adding deeper Office parsing and MCP integration.
