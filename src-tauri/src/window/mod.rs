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
pub mod geometry;
pub mod positioning;

pub const WINDOW_WILL_SHOW_EVENT: &str = "buddy:window-will-show";
pub const WINDOW_WILL_HIDE_EVENT: &str = "buddy:window-will-hide";

const COMPACT_AFTER_IDLE: Duration = Duration::from_secs(10 * 60);

/// 发布版默认只写 Warn 日志；窗口诊断使用这一入口保证每次呼出都能落盘。
/// 调试版仍使用 Info，避免把正常呼出显示为告警。
pub fn log_diagnostic(message: &str) {
    #[cfg(debug_assertions)]
    log::info!("{message}");
    #[cfg(not(debug_assertions))]
    log::warn!("{message}");
}

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
    trace_id: Option<u64>,
    emitted_at_ms: i64,
}

pub fn emit_window_will_show(
    window: &tauri::WebviewWindow,
    open_compact: bool,
    trace_id: Option<u64>,
) {
    let _ = window.emit(
        WINDOW_WILL_SHOW_EVENT,
        WindowWillShowPayload {
            open_compact,
            trace_id,
            emitted_at_ms: chrono::Utc::now().timestamp_millis(),
        },
    );
}

/// 记录一次用户呼出，并在闲置十分钟后要求前端先恢复紧凑气泡页。
pub fn notify_window_invoked(
    window: &tauri::WebviewWindow,
    allow_idle_compact: bool,
    trace_id: Option<u64>,
) -> bool {
    let state = window.app_handle().state::<WindowInvocationState>();
    let open_compact = state.mark_invoked_at(Instant::now(), allow_idle_compact);
    if open_compact {
        if let Err(error) = positioning::resize_window_to_page(window, "empty") {
            log::warn!("[window-perf] 闲置呼出切换气泡尺寸失败: {error}");
        }
    }
    emit_window_will_show(window, open_compact, trace_id);
    open_compact
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
