# IPC Contract

> Tauri `invoke`：前端 → 后端；单一 `stream-event`：后端 → 前端。参数在 TypeScript 使用 camelCase，Tauri 映射到 Rust snake_case。

## Invoke Commands

| Command | Arguments | Result |
|---|---|---|
| `send_message` | `{ messages: Message[], modelId: string }` | `void`；期间发送事件 |
| `stop_generation` | — | `void` |
| `approve_tool_call` | `{ id, approved, approveAll }` | `void` |
| `answer_tool_question` | `{ id, selected: number[], inputs?: string[], custom?: string \| null }` | `void` |
| `get_config` | — | `AppConfig` |
| `save_config` | `{ config: AppConfig }` | `void` |
| `fetch_models` | `{ baseUrl, apiKey, providerType? }` | `ModelInfo[]` |
| `test_latency` | `{ baseUrl, apiKey, modelId, providerType? }` | latency ms |
| `load_messages` | `{ offset, limit }` | chronological `Message[]` |
| `save_message` | `{ message }` | `void` |
| `resize_window_to_page` | `{ page }` | `void` |
| `log_window_frontend_diagnostic` | `{ traceId, stage, emittedAtMs, phaseElapsedMs }` | `void`；只接受预定义呼出阶段 |

`save_config` 会校验默认模型并重新注册热键。配置中的模型标识采用 `provider_id::原始模型ID`；读取旧配置时同步迁移模型、启用列表和默认选择，实际 Provider 请求仍使用原始模型 ID。`send_message` 由后端持久化新 user message，以及每一轮 assistant/tool message。

## `stream-event`

前端只监听：

```ts
listen<StreamEvent>('stream-event', ({ payload }) => {
  // 按 payload.event 分发
})
```

事件联合类型：

```ts
type StreamEvent =
  | { event: 'start' }
  | { event: 'text_start'; content_index: number }
  | { event: 'text_delta'; content_index: number; delta: string }
  | { event: 'text_end'; content_index: number; content: string }
  | { event: 'thinking_start'; content_index: number }
  | { event: 'thinking_delta'; content_index: number; delta: string }
  | { event: 'done'; reason: 'stop'|'error'|'aborted'; full_text: string }
  | { event: 'error'; reason: 'stop'|'error'|'aborted'; message: string; partial_text: string }
  | { event: 'tool_call_start'; id: string; name: string; content_index: number }
  | { event: 'tool_call_delta'; id: string; arguments_delta: string }
  | { event: 'tool_call_end'; id: string; name: string; arguments: string }
  | { event: 'tool_executing'; id: string; name: string }
  | { event: 'tool_result'; id: string; name: string; content: string; is_error: boolean }
  | { event: 'tool_approval_required'; id: string; name: string; arguments: string; reason: string }
  | { event: 'tool_question_required'; id: string; name: string; question: string;
      options: QuestionOption[]; multi_select: boolean; header: string }
  | { event: 'turn_end'; tool_calls_pending: number };
```

Rust 当前没有发射 `thinking_end`，但前端联合类型保留了该分支以兼容协议演进。

## Tool Flow

```text
send_message
  → tool_call_start/delta/end
  → [write tool] tool_approval_required
       → approve_tool_call
  → [ask_user] tool_question_required
       → answer_tool_question
  → tool_executing → tool_result → turn_end
  → 下一轮模型请求，或 done/error
```

同一轮 tool calls 顺序执行。`approveAll=true` 只在当前 `send_message` 调用中有效。

## Errors and Cancellation

Provider 错误统一转为 `{event:'error', message, partial_text}`，前端保留已生成内容并结束流式状态。`stop_generation` 通过 watch channel 请求取消，最终停止原因是 `aborted`；隐藏窗口不会调用它。
