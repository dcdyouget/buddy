# Task 04: Rust API Client & SSE

## 目标

实现 HTTP 客户端：流式聊天、获取模型列表、测速，以及 SSE 解析器。

## 相关设计文档

- `docs/design/sse-and-api.md` — 完整实现规范
- `docs/design/ipc-contract.md` — 事件名称约定

## 验收标准

- [ ] `stream_chat()` 可发送 POST 请求到 OpenAI 兼容 API
- [ ] SSE `data:` 行正确解析，`[DONE]` 正确处理
- [ ] 每个 token delta 通过 `app.emit("stream-token", delta)` 推送到前端
- [ ] 流结束时 `app.emit("stream-done", ())`
- [ ] 错误时 `app.emit("stream-error", message)`
- [ ] Cancel 信号通过 `watch::channel` 正确中断流
- [ ] `fetch_models()` 可解析 OpenAI 兼容 `/models` 响应
- [ ] `test_latency()` 返回正确答案的毫秒数
- [ ] HTTP 401/429/5xx 映射到正确的 `ApiError` 变体

## 开发工作

| ID | Task | File | Details |
|----|------|------|---------|
| D10 | SSE streaming client | `src-tauri/src/api.rs` | `stream_chat()` — POST, parse SSE, emit events per token |
| D11 | Stop generation | `src-tauri/src/api.rs` | `watch::channel` cancel mechanism |
| D12 | Fetch model list | `src-tauri/src/api.rs` | `fetch_models()` — GET /models |
| D13 | Speed test | `src-tauri/src/api.rs` | `test_latency()` — minimal request, measure TTFB |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T02 | SSE parser tests | Test with real SSE chunks, empty chunks, `[DONE]`, malformed JSON |
| T04 | Error mapping tests | Mock HTTP responses: 401 → Unauthorized, 429 → QuotaExceeded, 500 → ServerError(500), timeout → NetworkError |

在 `api.rs` 底部写 `#[cfg(test)] mod tests`，覆盖：

1. 单行 SSE data → 正确提取 delta
2. 多行合并在一个 chunk → 逐行解析
3. `data: [DONE]` → 返回 Ok
4. 空 data → 不 panic
5. JSON 格式错误 → 跳过该行，继续处理
6. Cancel 信号 → `select!` 正确退出
