# Task 07: Frontend Infrastructure

## 目标

创建 TypeScript 类型定义、Zustand 状态管理 stores、Tauri IPC 绑定。

## 相关设计文档

- `docs/design/rust-data-models.md` — 类型需与 Rust struct 对应
- `docs/design/ipc-contract.md` — invoke 调用和 event 监听规范

## 验收标准

- [ ] `types/index.ts` 包含所有与 Rust 对应的 TS 类型
- [ ] `configStore` 可读写配置（invoke get/save_config）
- [ ] `chatStore` 管理消息数组、发送/停止/追加/完成、草稿输入
- [ ] `uiStore` 管理当前页面、streaming 状态、错误信息、主题
- [ ] 所有 IPC 调用通过 store，不直接在组件中 invoke

## 开发工作

| ID | Task | File | Details |
|----|------|------|---------|
| D28 | TypeScript types | `src/types/index.ts` | `AppConfig`, `ProviderConfig`, `ModelInfo`, `Message`, `MessageRole`, `Theme`, `PageState` |
| D29 | configStore | `src/stores/configStore.ts` | `config`, `loadConfig()`, `saveConfig()`, `updateTheme()`, `addProvider()`, `toggleModel()` |
| D30 | chatStore | `src/stores/chatStore.ts` | `messages`, `sendMessage()`, `stopGeneration()`, `appendToken()`, `finalizeMessage()`, `draftInput` |
| D31 | uiStore | `src/stores/uiStore.ts` | `currentPage`, `isStreaming`, `streamingTokens`, `error`, `setPage()`, `setError()` |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T06 | Store logic tests | Mock IPC calls, test config load/save, message append/finalize, page transitions |

使用 vitest mock `@tauri-apps/api/core` 的 `invoke` 函数，验证：

1. `configStore.loadConfig()` 正确解析返回的 AppConfig
2. `chatStore.sendMessage()` 正确追加 user message + 触发 invoke
3. `chatStore.appendToken()` 正确追加到 assistant message 末尾
4. `chatStore.finalizeMessage()` 正确标记消息完成
5. `uiStore.setPage()` 正确切换页面并处理非法值
