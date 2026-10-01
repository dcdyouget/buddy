# Pages & State Machine

> **部分退役（S05-18，2026-09-29）**
>
> - 已实现并删除：State vs Component 表、EmptyPage、NoApiKeyPage、State Flow、Known Implementation Notes、模型下拉（S05-15，`crates/ui/src/chat/model_menu.rs`）、ChatPage 的审批 / 提问（S05-13，`ask_card.rs` / `approval_panel.rs`） —— 转换条件与 why 见 `crates/ui/src/chat/page_state.rs` 模块文档，路由与 engine 接入见 `crates/ui/src/chat/router.rs`
> - 设置骨架由 S06-01 实现，overlay 与窗口尺寸约束见 `crates/ui/src/settings/` 和 `crates/ui/src/chat/router.rs`；本段已部分退役。
> - 新增 Provider 流程由 S06-02 替代，理由见对应 spec 决策记录，代码见 `crates/ui/src/settings/provider/` 与 `crates/ui/src/chat/router_settings.rs`。
> - 模型列表、上下文与能力配置由 S06-03 替代，理由见对应 spec，代码见 `crates/ui/src/settings/model_list/` 与 `crates/ui/src/settings/model_config.rs`。
> - 未实现（保留）：设置子项（S06-04 至 S06-06）、Global Interactions（S07-*）
>
> 页面状态定义于 `src/types/index.ts`（v1，`v1-final`）；v2 的页面枚举是 `page_state::Page`。

## Pages

### SettingsPage（S06-*）

剩余设置子项为主题与快捷键。骨架、新增 Provider 与模型编辑不再在此重复定义。

设置操作通过 config store 即时保存。

设置页既可从输入区进入，也可由系统菜单栏“设置…”直接打开；菜单栏“开机自启”使用原生勾选状态并与 `config.auto_start` 同步。

## Global Interactions（S07-*）

- Esc：隐藏 Tauri 窗口，不停止生成。
- 失焦：由 Rust window event 隐藏窗口，不停止生成。
- 全局快捷键：默认 `CmdOrCtrl+J`，切换显示/隐藏并按光标屏幕定位。
- 外部选中文本：后端发 `selected-text`，前端在 empty/noapikey/conversation 时写入 draft。
- 主题：通过 `html.dark` 切换 CSS variables。
