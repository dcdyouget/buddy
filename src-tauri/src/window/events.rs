// 窗口事件处理模块：失焦时保存窗口位置
//
// 注：早期版本曾在 `Focused(false)` 时自动隐藏窗口并加 200ms 冷却，
// 但这会让"前台锁定下 show 失败的窗口被立刻又隐藏掉"，加重"按两次快捷键"问题。
// 现在改为只保存位置，窗口隐藏完全由快捷键 toggle 触发。

use log::info;
use tauri::Manager;

/// 设置窗口失焦事件处理器：失焦时保存窗口位置（按当前所在显示器分组）
///
/// 同一显示器上的位置会覆盖更新，方便用户在不同屏幕上分别记忆窗口位置。
pub fn setup_window_event_handler(
    window: &tauri::Window,
    event: &tauri::WindowEvent,
) {
    if let tauri::WindowEvent::Focused(false) = event {
        // 失焦时只保存位置，不隐藏窗口
        if let Ok(pos) = window.outer_position() {
            if let Ok(Some(mon)) = window.current_monitor() {
                let mp = mon.position();
                if let Some(state) = window
                    .app_handle()
                    .try_state::<crate::window::positioning::SavedWindowPositions>(
                ) {
                    if let Ok(mut map) = state.0.lock() {
                        map.insert((mp.x, mp.y), (pos.x, pos.y));
                        info!(
                            "[on_window_event] Focused(false), saved pos=({},{}) for monitor=({},{})",
                            pos.x, pos.y, mp.x, mp.y
                        );
                    }
                }
            }
        }
    }
}