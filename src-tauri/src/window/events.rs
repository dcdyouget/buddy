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

/// 设置窗口失焦事件处理器：失焦时保存位置并隐藏窗口（200ms 冷却）
pub fn setup_window_event_handler(
    window: &tauri::Window,
    event: &tauri::WindowEvent,
) {
    if let tauri::WindowEvent::Focused(false) = event {
        let now = Instant::now();
        let last_show = window
            .app_handle()
            .try_state::<LastShowTime>()
            .and_then(|s| s.0.lock().ok().map(|t| *t));
        let in_cooldown = last_show
            .map(|t| now.duration_since(t) < std::time::Duration::from_millis(200))
            .unwrap_or(false);
        info!(
            "[on_window_event] Focused(false), in_cooldown={}, last_show={:?}",
            in_cooldown,
            last_show.map(|t| format!("{:?} ago", now.duration_since(t)))
        );
        if in_cooldown {
            return;
        }
        // 隐藏前保存窗口位置
        if let Ok(pos) = window.outer_position() {
            if let Ok(Some(mon)) = window.current_monitor() {
                let mp = mon.position();
                if let Some(state) = window.app_handle().try_state::<crate::window::positioning::SavedWindowPositions>() {
                    if let Ok(mut map) = state.0.lock() {
                        map.insert((mp.x, mp.y), (pos.x, pos.y));
                        info!("[on_window_event] saved pos=({},{}) for monitor=({},{})", pos.x, pos.y, mp.x, mp.y);
                    }
                }
            }
        }
        info!("[on_window_event] hiding window");
        let _ = window.hide();
        info!("[on_window_event] hidden, visible={}", window.is_visible().unwrap_or(false));
    }
}
