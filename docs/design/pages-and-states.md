# Pages & State Machine

> **部分退役（S05-18，2026-09-29）**
>
> - 已实现并删除：State vs Component 表、EmptyPage、NoApiKeyPage、State Flow、Known Implementation Notes、模型下拉（S05-15，`crates/ui/src/chat/model_menu.rs`）、ChatPage 的审批 / 提问（S05-13，`ask_card.rs` / `approval_panel.rs`） —— 转换条件与 why 见 `crates/ui/src/chat/page_state.rs` 模块文档，路由与 engine 接入见 `crates/ui/src/chat/router.rs`
> - 设置骨架由 S06-01 实现，overlay 与窗口尺寸约束见 `crates/ui/src/settings/` 和 `crates/ui/src/chat/router.rs`；本段已部分退役。
> - 新增 Provider 流程由 S06-02 替代，理由见对应 spec 决策记录，代码见 `crates/ui/src/settings/provider/` 与 `crates/ui/src/chat/router_settings.rs`。
> - 模型列表、上下文与能力配置由 S06-03 替代，理由见对应 spec，代码见 `crates/ui/src/settings/model_list/` 与 `crates/ui/src/settings/model_config.rs`。
> - 热键录制与仅浅 / 深的主题控件由 S06-05 / S06-06 替代，配置先写盘后发布，主题经 `Theme` 全局刷新。理由见对应 spec，代码见 `crates/ui/src/settings/` 与 `crates/ui/src/chat/router_preferences.rs`。
> - 热键切换、外部选区与 Esc / 外点隐藏由 S07-03 / S07-04 替代，理由见对应 spec，代码见 `crates/ui/src/shell/runtime.rs`、`selection.rs`、`visibility.rs`。旧失焦隐藏描述不符合当前 v1，v2 按硬约束 7 监听真实外点。
> - 每屏运行期位置与底边锚定由 S07-06 替代，焦点屏 / 鼠标屏来源及 full frame / work area 区别见 spec 决策记录。
> - 未实现（保留）：系统菜单栏设置入口（S07-09）
>
> 页面状态定义于 `src/types/index.ts`（v1，`v1-final`）；v2 的页面枚举是 `page_state::Page`。

## Pages

### 系统菜单栏设置入口（S07-*）

设置页既可从输入区进入，也可由系统菜单栏“设置…”直接打开；菜单栏“开机自启”使用原生勾选状态并与 `config.auto_start` 同步。

