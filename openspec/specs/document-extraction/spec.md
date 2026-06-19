# document-extraction Specification

## Purpose
TBD - created by archiving change enhance-document-extraction-citations-observability. Update Purpose after archive.
## Requirements
### Requirement: Office 文档正文抽取
系统 SHALL 在导入 `.docx` 和 `.pptx` 时从 Office Open XML 内容中抽取可读正文，并将非空正文写入本地 chunk 索引。

#### Scenario: 导入 docx 正文
- **WHEN** 用户导入包含正文段落的 `.docx` 文件
- **THEN** 系统 MUST 创建可通过 `ask` 检索到的文本 chunk。

#### Scenario: 导入 pptx 幻灯片正文
- **WHEN** 用户导入包含多页幻灯片文本的 `.pptx` 文件
- **THEN** 系统 MUST 为幻灯片正文创建 chunk，并记录对应 `slide` 编号。

### Requirement: 待 AI/OCR 资产登记
系统 SHALL 对图片和无法直接抽取正文的业务资料保留 artifact 引用和待处理状态，且不得在默认导入流程中自动发送给外部 AI。

#### Scenario: 图片导入不外呼 AI
- **WHEN** 用户导入包含图片的目录且未显式请求 AI 处理
- **THEN** 系统 MUST 登记该图片文档，并且 MUST NOT 发起 AI provider 请求。

#### Scenario: 空正文文档保留文档记录
- **WHEN** 文档被发现但抽取正文为空
- **THEN** 系统 MUST 保留文档记录，并标记为待 AI/OCR 或无可索引正文状态。

### Requirement: 分片保留来源位置
系统 SHALL 在生成 chunk 时保留可用的来源位置，包括页码、幻灯片号、source range 或 artifact path。

#### Scenario: PDF 页码或基础位置可保留
- **WHEN** 抽取器能识别文档页码或位置
- **THEN** 系统 MUST 将该位置写入 chunk 元数据。

#### Scenario: PPT chunk 保留 slide 编号
- **WHEN** `.pptx` 抽取器从第 N 页幻灯片获得文本
- **THEN** 系统 MUST 将对应 chunk 的 `slide` 记录为 N。

### Requirement: 显式图片描述入库
系统 SHALL 仅在用户显式开启图片描述导入时，调用 AI provider 生成图片描述，并将描述作为可检索 chunk 写入索引。

#### Scenario: 默认导入图片不外呼 AI
- **WHEN** 用户执行 `ingest` 且未传入图片描述选项
- **THEN** 系统 MUST 只登记图片文档，且 MUST NOT 调用 AI provider。

#### Scenario: 图片描述写入索引
- **WHEN** 用户执行 `ingest --describe-images` 且图片描述 provider 调用成功
- **THEN** 系统 MUST 写入一个 `ai_generated = true` 的图片描述 chunk，并记录图片 artifact 路径。

#### Scenario: 图片描述 dry-run 不写 chunk
- **WHEN** 用户执行 `ingest --describe-images --dry-run-ai`
- **THEN** 系统 MUST 记录 AI dry-run 审计和 trace，且 MUST NOT 写入图片描述 chunk。

