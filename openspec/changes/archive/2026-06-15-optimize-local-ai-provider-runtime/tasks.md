# 任务：AI Provider Runtime 优化

## 已完成

- [x] 将主要 provider 入口收敛为 `mock` 和 `http`。
- [x] 保留 `openai-compatible` 作为旧配置别名。
- [x] 新增 `[ai.headers]` 配置解析。
- [x] 支持 `${ENV_NAME}` 请求头占位符。
- [x] 保留 `api_key_env` 兼容快捷方式。
- [x] 使用通用 HTTP provider 处理问答、embedding 和多模态图片请求。
- [x] 多模态请求复用同一套 `base_url` 和 headers。
- [x] 请求头值不进入审计、trace 或缓存 key。
- [x] `base_url` 改为只校验 `http`/`https` scheme，不按 provider 名称限制 localhost。
- [x] 删除旧的专用 provider 模块。
- [x] 更新 README、操作手册、数据文档、架构文档和 OpenSpec 规范。
- [x] 增加配置、provider descriptor、请求头展开、多模态 headers 的测试。

## 验证

- [x] `cargo test`
- [x] `cargo fmt -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `openspec validate --all`
