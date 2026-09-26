# Pages & State Machine

> 页面状态定义于 `src/types/index.ts`，实际路由位于 `src/App.tsx`。

## State vs Component

| `PageState` | Rendered component | Notes |
|---|---|---|
| `empty` | `EmptyPage` | 初始输入态 |
| `noapikey` | `NoApiKeyPage` | 配置不足时发送消息才进入 |
| `conversation` | `ChatPage` | 非流式聊天 |
| `streaming` | `ChatPage` | 同一组件，主要以 `chatStore.isStreaming` 判断 UI |
| `settings` | 背景页 + `SettingsPage` overlay | 保留 `previousPage`，不卸载聊天页 |
| `add-provider` | 无独立根路由 | 类型仍保留；实际由 Settings 内部 `showAddProvider` 控制侧滑面板 |

模型下拉菜单也不是根页面，而是 Empty/Chat 内部的 popover state。紧凑窗口
主动打开菜单时会只增加窗口高度，为浮层提供可见空间。

## Pages

### EmptyPage

使用 `InputDock` 显示输入、当前模型和发送按钮。发送时：

1. 空白内容忽略。
2. Provider 或默认模型缺失 → `noapikey`。
3. 配置有效 → `sendMessage`，随后设为 `streaming`。

窗口初始配置为 460×78，输入框上方提供展开箭头。点击箭头会进入 conversation；气泡状态进入设置后，返回也会进入 conversation 并恢复 750×500 的对话尺寸。

### NoApiKeyPage

显示“请先设置 API Key”，点击进入 Settings。配置变得有效后进入展开的 `conversation`。

### ChatPage

conversation 与 streaming 共用一个组件：

- 消息列表隐藏 role=tool 的内部消息。
- 带 `parent_message_id` 的 user message 嵌套在对应 assistant 下。
- 流式文本经 rAF 缓冲平滑渲染。
- 支持 thinking block、tool 状态、write tool 审批、`ask_user` 提问。
- 流式中 InputDock 显示停止操作；非流式时可发送和切换模型。
- 用户离开底部时不强制自动滚动；流式结束后可手动回到底部。

### SettingsPage

作为右侧 overlay 叠加在原页面上，包含主题、快捷键、模型列表和 FooterActions。添加 Provider 使用第二层 `AddProviderPanel` 侧滑层，支持预设、Base URL、API Key、获取模型、测速和添加模型。

设置操作通过 config store 即时保存；添加 Provider 会分步保存 provider、模型和默认模型，期间保持设置页不变，全部成功后返回展开的 `conversation`。

设置页既可从输入区进入，也可由系统菜单栏“设置…”直接打开；菜单栏“开机自启”使用原生勾选状态并与 `config.auto_start` 同步。

## State Flow

```text
startup ── load config/history ──> empty

empty ── send without config ──> noapikey ── click ──> settings
empty ── valid send ──> streaming ── done/error/stop ──> conversation
conversation ── send ──> streaming

empty/conversation/streaming/noapikey
  ── settings ──> overlay(previousPage)
settings ── back/cancel/confirm ──> previousPage
settings ── add ──> AddProviderPanel overlay ── cancel ──> settings
settings ── add success ──> conversation
```

## Global Interactions

- Esc：隐藏 Tauri 窗口，不停止生成。
- 失焦：由 Rust window event 隐藏窗口，不停止生成。
- 全局快捷键：默认 `CmdOrCtrl+J`，切换显示/隐藏并按光标屏幕定位。
- 外部选中文本：后端发 `selected-text`，前端在 empty/noapikey/conversation 时写入 draft。
- 主题：通过 `html.dark` 切换 CSS variables。

## Known Implementation Notes

- 启动加载历史消息后仍进入 `empty`；不会根据已有历史自动进入 conversation。
- `PageState.add-provider` 目前是遗留类型值，路由器没有对应分支。
- `uiStore.setPage` 会在 compact → content 时主动 resize，与项目“页面切换不改变窗口尺寸”的原始硬约束冲突。
