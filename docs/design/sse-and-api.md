# Streaming & Provider Integration

> **部分退役（RULES §7.2）**
> 已实现：S02-01（Provider Model / Compatibility / 两协议解析，`crates/engine/src/providers/`）；S02-05（Provider 层取消：响应头等待与读流两处，测试 `crates/engine/tests/stream_cancel.rs`）。
> 未实现：Streaming Flow 编排、工具循环上限、Context Policy、图片随工具轮携带、审批/提问/工具边界的取消、Error Model 的终态发射 → **S02-07**；Unified Events 的 UI 消费 → **S02-06**。三者完成后整份删除。

## Provider Model

后端通过 `LlmProvider` 统一两类协议：

- `openai_compatible`：OpenAI、DeepSeek、OpenRouter、GLM、MiniMax、Moonshot、Qwen、智谱及自定义兼容端点。
- `anthropic`：Anthropic Messages API。

前端预设位于 `src/types/index.ts`。实际请求由 `ProviderConfig.base_url`、`provider_type` 和可选 `compat` 决定，不假设所有厂商完全兼容 OpenAI。

## Compatibility

OpenAI-compatible 适配器会根据 `CompatConfig` 调整 thinking 格式、max token 字段、usage stream options、temperature 和 tools 等能力。API endpoint 会基于 base URL 组合模型、聊天路径；Anthropic 使用独立鉴权与事件解析。

## Streaming Flow

```text
send_message(messages, model_id)
  1. 读取模型与 Provider 配置
  2. 按 context_window × 70% 估算并裁剪最旧消息
  3. 原子占用生成任务，建立取消通道与 StreamEventEmitter
  4. 将本轮 user message 放入待保存列表
  5. 调 Provider.stream_chat
  6. 收集 assistant 内容（text/thinking/tool_calls）
  7. 无 tool_calls：结束循环
     有 tool_calls：审批/执行/收集 tool result，再进入下一轮
  8. 批量保存已有记录，释放生成占用，发送 done/error
```

工具循环硬上限 20 轮；连续 3 轮全部工具失败会提前中止。当前工具顺序执行，并发数为 1。

当模型属于 `openai_compatible` 且 `supports_image_generation=true` 时，本轮额外注册 `generate_image`。模型先返回 function tool call，Buddy 再调用同一 Provider 的生图接口：通用 Provider 使用 `/images/generations` 与当前模型 ID；MiniMax Provider 自动改用原生 `/image_generation`、`image-01` 和同一 API Key。生成图片通过 `tool_result.images` 发给前端并随 tool 消息持久化，回传聊天模型的 tool 文本不包含 Base64。Anthropic Messages 路径不注册该工具。

## Unified Events

所有 Provider 都输出 `stream-event`，内容包括 text/thinking block 生命周期、完成/错误、tool call 参数增量、审批、提问、执行结果和 turn end。前端通过 `useStreaming` 按到达顺序入队到 `chatStore`，文本增量由 `requestAnimationFrame` 平滑消费，块结束与工具边界随后按顺序处理。每轮 assistant 与 tool result 独立保留，最终答案不会合并进带 tool_calls 的消息。

## Cancellation

每次发送创建 `tokio::sync::watch` channel，`CancelState` 保存 sender。`stop_generation` 发送 `true`，在请求响应头、读取流、审批/提问和每轮工具循环边界检查取消状态。取消或流错误均终止本次工具循环。Esc、窗口失焦和托盘隐藏不触发取消。

## Context Policy

按字符数粗略估算 token，每轮 tool loop 都重新裁剪；保留最近消息，预算为模型上下文窗口的 70%。当前无 system prompt。

图片附件在聊天记录中只保存本地路径。发送带图消息时，后端临时读取当前用户
消息的图片并转换为 Provider 所需的 Base64 内容块；后续新的用户提问不会重复
发送历史图片。若同一轮发生工具调用，由于 Provider API 无状态，当前图片会在
该轮后续请求中继续携带，以保持工具循环上下文。

## Model List and Latency

- `fetch_models(base_url, api_key, provider_type)`：由对应 Provider 获取模型并补充上下文窗口。
- `test_latency(base_url, api_key, model_id, provider_type)`：由对应 Provider 发送最小请求并返回毫秒值。
- 未知 `provider_type` 回退为 OpenAI-compatible；未知模型上下文默认 128000。

## Error Model

后端 `ApiError` 统一覆盖未授权、配额、服务端、网络和流解析错误。Provider 返回部分输出及终止原因，由 command 层保存已有记录、释放当前生成占用后，统一发送一次 done 或 error，避免界面提前结束而后台仍执行工具。
