# Pages & State Machine

> **部分退役（S05-18，2026-09-29）**
>
> - 已实现并删除：State vs Component 表、EmptyPage、NoApiKeyPage、State Flow、Known Implementation Notes、模型下拉（S05-15，`crates/ui/src/chat/model_menu.rs`） —— 转换条件与 why 见 `crates/ui/src/chat/page_state.rs` 模块文档，路由与 engine 接入见 `crates/ui/src/chat/router.rs`
> - 未实现（保留）：ChatPage 的审批 / 提问（S05-13）、SettingsPage（S06-*）、Global Interactions（S07-*）
>
> 页面状态定义于 `src/types/index.ts`（v1，`v1-final`）；v2 的页面枚举是 `page_state::Page`。

## Pages

### ChatPage（剩余部分）

- 支持 write tool 审批、`ask_user` 提问（S05-13）。
- 带 `parent_message_id` 的 user message 嵌套在对应 assistant 下（已由行模型实现，S05-02）。

### SettingsPage（S06-*）

作为右侧 overlay 叠加在原页面上，包含主题、快捷键、模型列表和 FooterActions。添加 Provider 使用第二层 `AddProviderPanel` 侧滑层，支持预设、Base URL、API Key、获取模型、测速和添加模型。

设置操作通过 config store 即时保存；添加 Provider 会分步保存 provider、模型和默认模型，期间保持设置页不变，全部成功后返回展开的 `conversation`。

设置页既可从输入区进入，也可由系统菜单栏“设置…”直接打开；菜单栏“开机自启”使用原生勾选状态并与 `config.auto_start` 同步。

## Global Interactions（S07-*）

- Esc：隐藏 Tauri 窗口，不停止生成。
- 失焦：由 Rust window event 隐藏窗口，不停止生成。
- 全局快捷键：默认 `CmdOrCtrl+J`，切换显示/隐藏并按光标屏幕定位。
- 外部选中文本：后端发 `selected-text`，前端在 empty/noapikey/conversation 时写入 draft。
- 主题：通过 `html.dark` 切换 CSS variables。
