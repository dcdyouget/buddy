// 窗口事件处理模块：失焦隐藏、冷却时间管理

use std::sync::Mutex;
use std::time::Instant;
use log::info;
use tauri::Manager;

/// 记录窗口最后 show 的时间戳，用于失焦冷却
pub struct LastShowTime(pub Mutex<Instant>);

/// 标记窗口刚被 show，更新失焦冷却时间戳
pub fn mark_window_shown(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LastShowTime>() {
        if let Ok(mut t) = state.0.lock() {
            *t = Instant::now();
            info!("[mark_window_shown] timestamp updated");
        }
    }
}

/// 设置窗口失焦事件处理器：失焦时保存位置（不再隐藏窗口，
/// 正常被其他窗口遮挡；仅热键触发 show/hide toggle）
pub fn setup_window_event_handler(
    window: &tauri::Window,
    event: &tauri::WindowEvent,
) {
    if let tauri::WindowEvent::Focused(false) = event {
        // 失焦时只保存位置，不隐藏窗口
        if let Ok(pos) = window.outer_position() {
            if let Ok(Some(mon)) = window.current_monitor() {
                let mp = mon.position();
                if let Some(state) = window.app_handle().try_state::<crate::window::positioning::SavedWindowPositions>() {
                    if let Ok(mut map) = state.0.lock() {
                        map.insert((mp.x, mp.y), (pos.x, pos.y));
                        info!("[on_window_event] Focused(false), saved pos=({},{}) for monitor=({},{})", pos.x, pos.y, mp.x, mp.y);
                    }
                }
            }
        }
    }
}
