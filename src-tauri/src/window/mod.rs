// 窗口模块
//
// 职责：
// - `events`   —— 监听窗口事件（聚焦、失焦、关闭、移动），处理焦点切换时的快捷键冷却等逻辑
// - `positioning` —— 记忆与恢复窗口在屏幕坐标系中的位置；提供 SavedWindowPositions 跨命令共享
//
// 这两个模块被 `lib.rs` 的 `on_window_event` 闭包和 `invoke_handler` 调用。

pub mod events;
pub mod positioning;
