// 窗口模块
//
// 职责：
// - `events`   —— 监听窗口事件（聚焦、失焦、关闭、移动），处理焦点切换时的快捷键冷却等逻辑
// - `positioning` —— 记忆与恢复窗口在屏幕坐标系中的位置；提供 SavedWindowPositions 跨命令共享
//
// 这两个模块被 `lib.rs` 的 `on_window_event` 闭包和 `invoke_handler` 调用。

use parking_lot::Mutex;
use serde::Serialize;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

pub mod events;
pub mod positioning;

pub const WINDOW_WILL_SHOW_EVENT: &str = "buddy:window-will-show";
pub const WINDOW_WILL_HIDE_EVENT: &str = "buddy:window-will-hide";

const COMPACT_AFTER_IDLE: Duration = Duration::from_secs(10 * 60);

pub struct WindowInvocationState {
    last_invoked_at: Mutex<Instant>,
}

impl Default for WindowInvocationState {
    fn default() -> Self {
        Self {
            last_invoked_at: Mutex::new(Instant::now()),
        }
    }
}

impl WindowInvocationState {
    fn mark_invoked_at(&self, now: Instant, allow_idle_compact: bool) -> bool {
        let mut last_invoked_at = self.last_invoked_at.lock();
        let should_open_compact = allow_idle_compact
            && now.saturating_duration_since(*last_invoked_at) >= COMPACT_AFTER_IDLE;
        *last_invoked_at = now;
        should_open_compact
    }
}

#[derive(Clone, Copy, Serialize)]
struct WindowWillShowPayload {
    open_compact: bool,
}

pub fn emit_window_will_show(window: &tauri::WebviewWindow, open_compact: bool) {
    let _ = window.emit(
        WINDOW_WILL_SHOW_EVENT,
        WindowWillShowPayload { open_compact },
    );
}

/// 记录一次用户呼出，并在闲置十分钟后要求前端先恢复紧凑气泡页。
pub fn notify_window_invoked(window: &tauri::WebviewWindow, allow_idle_compact: bool) {
    let state = window.app_handle().state::<WindowInvocationState>();
    let open_compact = state.mark_invoked_at(Instant::now(), allow_idle_compact);
    emit_window_will_show(window, open_compact);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_compact_only_after_ten_minutes_of_inactivity() {
        let started_at = Instant::now();
        let state = WindowInvocationState {
            last_invoked_at: Mutex::new(started_at),
        };

        assert!(!state.mark_invoked_at(started_at + Duration::from_secs(599), true));
        assert!(state.mark_invoked_at(started_at + Duration::from_secs(599 + 600), true));
    }

    #[test]
    fn explicit_settings_open_never_forces_compact_mode() {
        let started_at = Instant::now();
        let state = WindowInvocationState {
            last_invoked_at: Mutex::new(started_at),
        };

        assert!(!state.mark_invoked_at(started_at + Duration::from_secs(601), false));
    }
}
