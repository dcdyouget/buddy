# Buddy Multi-Model Support -- Implementation Plan

## 1. Current State Assessment

The backend refactoring (Rust provider layer) is complete. The `LlmProvider` trait, `StreamEvent` protocol, `StreamEventEmitter`, OpenAI-compatible and Anthropic providers are all implemented and wired into `commands.rs`. The Rust backend now emits unified `stream-event` payloads via the new `StreamEventEmitter`.

**The critical gap: the frontend has NOT been updated to consume the new protocol.** The frontend still listens for the old `stream-token`/`stream-done`/`stream-error`/`stream-cancelled` events and still uses flat-text `appendToken` with post-hoc `<think>` tag parsing.

### What is currently happening at runtime:

1. Frontend calls `invoke('send_message', ...)` as before.
2. `commands::send_message()` creates a `StreamEventEmitter` and calls `llm_provider.stream_chat()`.
3. The provider emits `stream-event` events with typed `StreamEvent` JSON payloads (e.g., `{"event":"text_delta","content_index":0,"delta":"hello"}`).
4. The frontend `useStreaming.ts` is **listening for the OLD event names** (`stream-token`, `stream-done`, etc.), so **no streaming tokens reach the frontend**.
5. When `send_message` returns `Err(ApiError::Unauthorized)`, it emits the OLD `stream-error` event (line 121 of commands.rs), but the provider itself also emits an `Error` event via `stream-event`. This causes dual error emission.

### What needs to change:

The frontend needs to be updated to:
1. Listen for the unified `stream-event` instead of the old four event names.
2. Parse `StreamEvent` JSON payloads and handle `text_delta`/`thinking_delta`/`done`/`error` separately.
3. Build structured `ContentBlock[]` during streaming and attach them to the message on completion.
4. Render from `blocks[]` when available, falling back to `<think>` tag text parsing for legacy messages.
5. Handle all error/cancellation cases through the unified `error` event path.

---

## 2. Event Protocol (StreamEvent) -- Reference

The Rust `StreamEvent` enum serializes with `#[serde(tag = "event", rename_all = "snake_case")]`, producing JSON like these:

| Rust Variant | JSON `event` field | Meaning |
|---|---|---|
| `Start` | `"start"` | Stream begins. Frontend should reset streaming state. |
| `TextStart{content_index}` | `"text_start"` | A text content block begins at index 0. |
| `TextDelta{content_index, delta}` | `"text_delta"` | Incremental text token. Append to current text block. |
| `TextEnd{content_index, content}` | `"text_end"` | Text block complete with final full content. |
| `ThinkingStart{content_index}` | `"thinking_start"` | Thinking/reasoning block begins. |
| `ThinkingDelta{content_index, delta}` | `"thinking_delta"` | Incremental thinking token. |
| `ThinkingEnd{content_index, content}` | `"thinking_end"` | Thinking block complete. |
| `Done{reason, full_text}` | `"done"` | Stream finished successfully. `reason` is one of: `"stop"`, `"length"`. |
| `Error{reason, message, partial_text}` | `"error"` | Stream errored. `reason` is `"aborted"`, `"error"`. `partial_text` contains any text accumulated before the error. |

The TypeScript types in `/Users/gongshaojie/Project/buddy/src/types/index.ts` already match this (lines 63-72).

**Current providers emit exactly these patterns:**

- **OpenAI-compatible:** Emits `Start` -> `TextStart(0)` -> `TextDelta(0, token)...` -> `TextEnd(0, full)` -> `Done`. If the model supports `reasoning_content` (DeepSeek), emits `ThinkingDelta(0, reasoning)...` interspersed with text deltas.
- **Anthropic:** Emits `Start` -> (for each content_block): `TextStart`/`ThinkingStart` -> `TextDelta`/`ThinkingDelta`... -> `TextEnd`/`ThinkingEnd`, then `Done`. Anthropic can have multiple content blocks (e.g., thinking block then text block), each with incrementing `content_index`.

---

## 3. Rust Backend Changes Required

### 3.1 Fix Dual Error Emission in `commands.rs`

**File:** `/Users/gongshaojie/Project/buddy/src-tauri/src/commands.rs`, lines 116-130

**Problem:** When `stream_chat()` returns `Err(...)`, the code emits the OLD `stream-error` event (which the frontend no longer listens for). The provider may have already emitted an `Error` event via `stream-event` before returning. The frontend now listens exclusively for `stream-event`.

**Fix:** Replace the old `app.emit("stream-error", ...)` with an `emitter.error(...)` call so all errors flow through the unified channel. Clone the AppHandle before the emitter is moved into `stream_chat()`.

```rust
// In commands.rs, before creating the emitter:
let app_handle_for_error = app.clone();

// Then in the Err branch, use a fresh emitter:
Err(e) => {
    warn!("[send_message] 请求失败: {}", e);
    let error_emitter = StreamEventEmitter::new(app_handle_for_error);
    let msg = e.to_string();
    error_emitter.error(StopReason::Error, &msg, "");
}
```

### 3.2 Remove Dead `api.rs` Module

**File:** `/Users/gongshaojie/Project/buddy/src-tauri/src/api.rs` (497 lines)

This legacy module is no longer used. `commands.rs` dispatches to `providers::create_provider()` instead of calling `api::stream_chat()` directly. The `test_latency` and `fetch_models` functions in `api.rs` are superseded by the provider implementations.

**Actions:**
- Remove the `mod api;` declaration from `/Users/gongshaojie/Project/buddy/src-tauri/src/lib.rs` (line 21)
- Delete `/Users/gongshaojie/Project/buddy/src-tauri/src/api.rs`

### 3.3 No Other Backend Changes Needed

The provider implementations (`openai_compatible.rs`, `anthropic.rs`), trait definition (`mod.rs`), streaming module (`streaming.rs`), models (`models.rs`), and storage (`storage.rs`) are all complete and correct. The `commands.rs` `send_message` function already correctly dispatches via `providers::create_provider(&provider_type)`.

---

## 4. Frontend Changes -- Step by Step

All frontend files are under `/Users/gongshaojie/Project/buddy/src/`.

### 4.1 Rewrite `useStreaming.ts` -- Listen for New `stream-event`

**File:** `/Users/gongshaojie/Project/buddy/src/hooks/useStreaming.ts`

**Current behavior:** Listens for four separate Tauri events (`stream-token`, `stream-done`, `stream-error`, `stream-cancelled`) and calls legacy `chatStore.appendToken()` or `chatStore.finalizeMessage()`.

**Required change:** Replace all four listeners with a single `stream-event` listener. Parse the JSON payload as `StreamEvent`, then dispatch to new `chatStore` methods by event type. Retain all error categorization logic (401 -> noapikey page, 429 -> quota message, etc.) but trigger it from `StreamEvent.Error.message`.

**Key implementation details:**

- Listen on `'stream-event'` with payload type `StreamEvent` (imported from `@/types`).
- Switch on `payload.event`:
  - `'start'` -> `chatStore.handleStreamStart()`
  - `'text_start'` -> `chatStore.handleTextStart(payload.content_index)`
  - `'text_delta'` -> `chatStore.handleTextDelta(payload.content_index, payload.delta)`
  - `'text_end'` -> `chatStore.handleTextEnd(payload.content_index, payload.content)`
  - `'thinking_start'` -> `chatStore.handleThinkingStart(payload.content_index)`
  - `'thinking_delta'` -> `chatStore.handleThinkingDelta(payload.content_index, payload.delta)`
  - `'thinking_end'` -> `chatStore.handleThinkingEnd(payload.content_index, payload.content)`
  - `'done'` -> `chatStore.handleStreamDone()` then `setPage('conversation')`
  - `'error'` -> `chatStore.handleStreamError(payload.reason, payload.message)`, then run existing error categorization logic (check for 401/429/5xx/network in message, navigate/insert warning messages accordingly). If `payload.reason === 'aborted'`, just `finalizeMessage()` and `setPage('conversation')`.
- Remove the `stream-cancelled` listener entirely. Cancellation is now an `error` event with `reason: "aborted"`.
- The epoch-based race condition prevention pattern (epochRef + dynamic import) should be preserved as-is.

### 4.2 Extend `chatStore.ts` -- Block-Based Content Management

**File:** `/Users/gongshaojie/Project/buddy/src/stores/chatStore.ts`

**Current behavior:** `appendToken(token)` appends to `messages[last].content` as a flat string. No notion of content blocks or thinking segments.

**Required change:** Add `streamingBlocks: ContentBlock[]` to state and add new handler methods for each StreamEvent variant. Keep the old `appendToken` and `finalizeMessage` for reference but the new flow uses the new methods.

**New state field:**
```typescript
streamingBlocks: ContentBlock[];  // Live blocks being built during streaming
```

**New methods to add to ChatState interface and implementation:**

1. `handleStreamStart()` -- Initialize/reset `streamingBlocks` to empty array.
2. `handleTextStart(contentIndex)` -- Ensure `streamingBlocks` array is large enough for this index, initialize/overwrite with `{ type: 'text', content: '' }`.
3. `handleTextDelta(contentIndex, delta)` -- Append `delta` to `streamingBlocks[contentIndex].content`. Also append to `messages[last].content` (flat string for backward compatibility and persistence).
4. `handleTextEnd(contentIndex, content)` -- Optionally set final content (usually already accumulated via deltas).
5. `handleThinkingStart(contentIndex)` -- Ensure array space, initialize with `{ type: 'thinking', content: '', is_open: true }`.
6. `handleThinkingDelta(contentIndex, delta)` -- Append delta to thinking block content.
7. `handleThinkingEnd(contentIndex, content)` -- Set final thinking content and `is_open: false`.
8. `handleStreamDone()` -- Attach `streamingBlocks` to the last assistant message as `blocks: ContentBlock[]`, then reset `isStreaming`/`streamingTokens`/`streamingModelId`/`streamingBlocks`.
9. `handleStreamError(reason, message)` -- Reset streaming state (`isStreaming`, `streamingTokens`, `streamingModelId`, `streamingBlocks`), store error message.

**Important design decision:** The flat `content` string on each Message is kept as the source of truth for persistence. The `blocks` field is a runtime annotation attached at stream completion. This means:
- Messages loaded from disk will NOT have `blocks` -- the frontend falls back to `<think>` tag text parsing (existing behavior).
- Messages created during the current session WILL have `blocks` for structured rendering.
- No Rust-side persistence changes needed.

### 4.3 Update `MessageBubble.tsx` -- Block-Aware Rendering with Legacy Fallback

**File:** `/Users/gongshaojie/Project/buddy/src/components/chat/MessageBubble.tsx`

**Current behavior:** Always calls `parseThinkBlocks(message.content)` to split text on `<think>` tags. Renders text segments via `StreamingMarkdown` and think segments via `ThinkSection`.

**Required change:** Check for `message.blocks` first. If blocks are present, render each block directly (skipping `<think>` tag parsing). If no blocks (legacy message from disk), fall back to the existing `parseThinkBlocks` path.

**New rendering logic structure:**

```
if (message.blocks && message.blocks.length > 0) {
  // New v2.0 path: render structured blocks directly
  blocks.map(block => {
    if (block.type === 'thinking') {
      <ThinkSection content={block.content} isStreaming={...} defaultExpanded={...} />
    } else {
      <StreamingMarkdown content={block.content} isStreaming={...} />
    }
  })
} else {
  // Legacy v1.0 path: parse <think> tags from flat text
  // ... existing parseThinkBlocks + segment rendering logic ...
}
```

**Memo comparator update:** The custom comparator in `React.memo` must also check `message.blocks`. Use a simple JSON.stringify comparison or add a separate check for `blocks` array equality.

**Streaming indicator:** The blinking cursor (`buddy-cursor`) should appear after the last block when `isStreaming` is true. The "回到问题" button should appear after all blocks when `isStreaming` is false and `questionId` is provided.

### 4.4 Update `StreamingPage.tsx` -- Pass Live Blocks During Streaming

**File:** `/Users/gongshaojie/Project/buddy/src/pages/StreamingPage.tsx`

**Current behavior:** Iterates `messages[]`, renders `MessageBubble` for each. The last assistant message has empty/partial `content` which is updated by `chatStore.appendToken`.

**Required change:** During streaming, the last assistant message in `messages[]` does NOT yet have `blocks` (the live blocks live in `streamingBlocks` state). The page must merge `streamingBlocks` into the last message before rendering.

**Implementation:** Before mapping messages to `MessageBubble`, compute a `displayMessages` array:

```typescript
const displayMessages = messages.map((msg, i) => {
  const isLast = i === messages.length - 1;
  if (isLast && msg.role === 'assistant' && isStreaming && streamingBlocks.length > 0) {
    return { ...msg, blocks: streamingBlocks };
  }
  return msg;
});
```

This is a minimal, non-invasive change. `ConversationPage.tsx` does NOT need this change because streaming is not active on that page.

### 4.5 Verify `ProviderCard.tsx` -- Provider Type Propagation

**File:** `/Users/gongshaojie/Project/buddy/src/components/settings/ProviderCard.tsx`

Verify that when a user selects a provider preset (including Anthropic), the `provider_type` field from the preset is properly carried into the `ProviderConfig` object passed to `configStore.addProvider()`. The `PROVIDER_PRESETS` array in `types/index.ts` already includes `provider_type: 'anthropic'` for the Anthropic preset. The fix, if needed, is a one-line change to ensure `provider_type` is included in the config object.

### 4.6 No Other Frontend File Changes Needed

The following files require NO changes:
- **`ConversationPage.tsx`** -- Messages are rendered by `MessageBubble`, which handles both block and legacy paths.
- **`configStore.ts`** -- `ProviderConfig` type already includes `provider_type`.
- **`uiStore.ts`** -- No streaming-related state.
- **`ThinkSection.tsx`** -- Accepts `content` string and `isStreaming` boolean; works identically regardless of source.
- **`StreamingMarkdown.tsx`** -- Accepts `content` string; unchanged.
- **`thinkParser.ts`** -- Kept as legacy fallback path for loaded messages without blocks.
- **`CodeBlock.tsx`** -- Unchanged.
- **`ModelDropdown.tsx`** -- Unchanged.
- **`InputDock.tsx`** -- Unchanged.
- **`SettingsPage.tsx`** -- No provider-type-specific UI needed beyond what `ProviderCard` and `PROVIDER_PRESETS` already provide.

---

## 5. Migration Path -- Ensuring Zero Breakage

### 5.1 Backward Compatibility Strategy

| Layer | Old Path | New Path | Migration Strategy |
|---|---|---|---|
| Frontend event listener | `stream-token` / `stream-done` / `stream-error` / `stream-cancelled` | `stream-event` with typed payload | **Hard cut.** The backend no longer emits old events (except the error fix in 3.1). The frontend switches to the new listener. No dual-listening period needed. |
| Frontend content model | Flat `content: string` | `content` + optional `blocks: ContentBlock[]` | **Dual support.** New rendering code checks `blocks` first, falls back to `parseThinkBlocks(content)` for messages without blocks. Messages loaded from disk (no blocks) seamlessly use the legacy path. |
| Backend config | `ProviderConfig` without `provider_type` | `ProviderConfig` with `provider_type` (defaults to `"openai_compatible"`) | **No migration.** Existing `config.json` files work without changes -- the Rust `#[serde(default = "default_provider_type")]` fills in `"openai_compatible"` for missing fields. |
| Backend streaming | `api::stream_chat()` (old SSE to raw events) | `providers::OpenAICompatibleProvider \| AnthropicProvider` (SSE to StreamEvent) | **Already migrated.** `commands.rs` dispatches based on `provider_type`. Old providers default to `openai_compatible`. |
| Message persistence | Flat `content` string in chunk files | Same flat `content` string | **No change.** Blocks are runtime-only. Persistence remains flat text. |

### 5.2 Implementation Order (Recommended Sequence)

1. **Phase 1 -- chatStore:** Add `streamingBlocks` state and the 9 new handler methods to `chatStore.ts`. This is the foundation.
2. **Phase 2 -- useStreaming:** Rewrite `useStreaming.ts` to listen for `stream-event` and call the new chatStore methods.
3. **Phase 3 -- MessageBubble:** Add block-aware rendering with legacy fallback. This is when streaming becomes visually functional.
4. **Phase 4 -- StreamingPage:** Merge `streamingBlocks` into last message for live display during streaming.
5. **Phase 5 -- Backend cleanup:** Fix `commands.rs` dual error emission, remove `api.rs` dead code.
6. **Phase 6 -- ProviderCard verification:** Ensure `provider_type` propagates correctly.
7. **Phase 7 -- End-to-end testing:** Test with both DeepSeek (OpenAI-compatible) and Anthropic providers.

---

## 6. File Change Summary

| File (absolute path) | Action | Expected Lines Changed | Complexity |
|---|---|---|---|
| `/Users/gongshaojie/Project/buddy/src-tauri/src/commands.rs` | Fix dual error emission -- replace `app.emit("stream-error")` with `StreamEventEmitter` error | ~15 changed | Low |
| `/Users/gongshaojie/Project/buddy/src-tauri/src/lib.rs` | Remove `mod api;` declaration | ~1 removed | Trivial |
| `/Users/gongshaojie/Project/buddy/src-tauri/src/api.rs` | **DELETE** -- dead code, superseded by providers | ~497 removed | None |
| `/Users/gongshaojie/Project/buddy/src/hooks/useStreaming.ts` | Rewrite -- listen for unified `stream-event`, dispatch to new chatStore methods | ~100 changed | Medium |
| `/Users/gongshaojie/Project/buddy/src/stores/chatStore.ts` | Add `streamingBlocks` state + 9 new handler methods | ~120 added | Medium |
| `/Users/gongshaojie/Project/buddy/src/components/chat/MessageBubble.tsx` | Add block-aware rendering with legacy `<think>` tag fallback | ~40 changed | Medium |
| `/Users/gongshaojie/Project/buddy/src/pages/StreamingPage.tsx` | Merge live `streamingBlocks` into last message for rendering | ~10 changed | Low |
| `/Users/gongshaojie/Project/buddy/src/components/settings/ProviderCard.tsx` | Verify `provider_type` propagation (likely no code change) | ~0-5 | Trivial |

**Total estimated: ~300 lines added/changed, ~500 removed (dead code).**

---

## 7. Testing Plan

### 7.1 Backend Unit Tests (Rust)

- **`streaming.rs`:** Add serialization tests for `Start`, `TextStart`, `TextEnd`, `ThinkingStart`, `ThinkingDelta`, `ThinkingEnd`, `Error` variants (only `TextDelta` and `Done` currently tested).
- **`providers/anthropic.rs`:** Add round-trip test: feed a known Anthropic SSE byte stream through the parser, verify the emitted StreamEvent sequence matches expectations.
- **`providers/openai_compatible.rs`:** Add test for OpenAI SSE parsing and DeepSeek `reasoning_content` extraction.

### 7.2 End-to-End Manual Verification

| Test Case | Steps | Expected Result |
|---|---|---|
| **OpenAI streaming** | Configure DeepSeek provider, send "你好", wait for response | Text streams token-by-token, completes normally |
| **Anthropic streaming** | Configure Anthropic provider, send "Hello", wait for response | Text streams token-by-token, completes normally |
| **Thinking blocks (DeepSeek R1)** | Send a reasoning problem to DeepSeek R1 | Collapsible "thinking" section appears, auto-collapses when complete |
| **Thinking blocks (Claude)** | Send a reasoning problem to Claude with extended thinking | Collapsible "thinking" section appears, auto-collapses when complete |
| **Cancel during streaming** | Send a message, click stop button mid-stream | Stream terminates, partial text preserved, returns to conversation page |
| **401 error** | Use an invalid API key, send message | "请配置 API Key" page is displayed |
| **429 error** | Use a quota-exhausted key, send message | Warning message shown in conversation |
| **Network error** | Disconnect from internet, send message | "网络错误，请重试" message shown |
| **Legacy messages** | Load old conversation with `<think>` tags in flat text | Messages render correctly with collapsible think sections via legacy path |
| **Model switching** | Switch from DeepSeek to Anthropic model mid-conversation | New messages use the correct provider |
| **Window resize** | Observe window size during streaming vs conversation | Window resizes smoothly between pages |
| **Code blocks during streaming** | Receive a response containing fenced code blocks | Syntax highlighting works, copy button works |
| **App restart** | Send messages, quit app, relaunch | All messages load correctly, rendered via legacy/block path as appropriate |

### 7.3 Edge Cases

- Empty content blocks (model returns no text): should not throw errors.
- Multiple content blocks from Anthropic (thinking block followed by text block): each gets its own `content_index`, rendered in order.
- Rapid token arrival: no token loss, no UI jank.
- Send while already streaming: prevented by `isStreaming` check in `InputDock`.
- Very long responses (1000+ tokens): stable/unstable split in `StreamingMarkdown` should handle this efficiently.

---

## 8. Architecture Diagram (Post-Implementation)

```
┌──────────────────────────────────────────────────────────────────┐
│                     Frontend (React + TypeScript)                │
│                                                                  │
│  useStreaming.ts ── listens for "stream-event"                   │
│       │                                                          │
│       ├─ start        → chatStore.handleStreamStart()            │
│       ├─ text_delta   → chatStore.handleTextDelta(idx, delta)    │
│       ├─ thinking_delta → chatStore.handleThinkingDelta(...)     │
│       ├─ done         → chatStore.handleStreamDone()             │
│       └─ error        → chatStore.handleStreamError + page nav   │
│                                                                  │
│  chatStore.ts                                                    │
│       │ streamingBlocks: ContentBlock[] (live during streaming)  │
│       │ messages[last].blocks: ContentBlock[] (set at done)      │
│       │ messages[last].content: string (flat, for persistence)   │
│       ▼                                                          │
│  MessageBubble.tsx                                               │
│       │ if blocks → render blocks directly                       │
│       │ else → parseThinkBlocks(content) [legacy]                │
│       ▼                                                          │
│  ThinkSection / StreamingMarkdown / CodeBlock                    │
├──────────────────────────────────────────────────────────────────┤
│                    Tauri IPC (commands.rs)                        │
│                                                                  │
│  send_message() → provider_type → create_provider()              │
│  fetch_models() → provider_type → create_provider()              │
│  test_latency() → provider_type → create_provider()              │
├──────────────────────────────────────────────────────────────────┤
│                  Provider Layer (providers/)                      │
│                                                                  │
│  trait LlmProvider {                                             │
│      fn stream_chat(..., emitter: &StreamEventEmitter)           │
│      fn fetch_models(...)                                        │
│      fn test_latency(...)                                        │
│  }                                                               │
│                                                                  │
│  ┌─────────────────────────┐  ┌──────────────────────────┐      │
│  │ OpenAICompatibleProvider │  │   AnthropicProvider       │      │
│  │                         │  │                           │      │
│  │ POST /chat/completions  │  │ POST /v1/messages         │      │
│  │ Auth: Bearer {key}      │  │ Auth: x-api-key {key}     │      │
│  │ SSE: data: {...}        │  │ SSE: event: ... data: ... │      │
│  │ reasoning_content →     │  │ thinking_delta →          │      │
│  │   thinking events        │  │   thinking events         │      │
│  └─────────────────────────┘  └──────────────────────────┘      │
│                        │                                         │
│                        ▼                                         │
│  StreamEventEmitter → emits "stream-event" Tauri events          │
│  with StreamEvent JSON payloads                                  │
│  (start / text_start / text_delta / thinking_delta / done / ...) │
└──────────────────────────────────────────────────────────────────┘
```
