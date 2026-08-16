// 窗口事件处理模块：移动或失焦时保存窗口位置
//
// 注：早期版本曾在 `Focused(false)` 时自动隐藏窗口并加 200ms 冷却，
// 但这会让"前台锁定下 show 失败的窗口被立刻又隐藏掉"，加重"按两次快捷键"问题。
// 现在改为只保存位置，窗口隐藏完全由快捷键 toggle 触发。

use log::info;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::Manager;

use crate::window::positioning::SavedWindowPositions;

const POSITION_SAVE_DEBOUNCE: Duration = Duration::from_millis(160);

/// 保存窗口位置（按当前所在显示器分组）。
///
/// 同一显示器上的位置会覆盖更新，方便用户在不同屏幕上分别记忆窗口位置。
fn save_window_position(window: &tauri::Window, moved_position: Option<(i32, i32)>, reason: &str) {
    let position = moved_position.or_else(|| {
        window
            .outer_position()
            .ok()
            .map(|position| (position.x, position.y))
    });
    let (x, y) = match position {
        Some(position) => position,
        None => return,
    };
    let monitor = match window.current_monitor() {
        Ok(Some(monitor)) => monitor,
        _ => return,
    };
    let monitor_position = monitor.position();
    let Some(state) = window.app_handle().try_state::<SavedWindowPositions>() else {
        return;
    };

    state
        .positions
        .lock()
        .insert((monitor_position.x, monitor_position.y), (x, y));
    info!(
        "[on_window_event] {}, saved pos=({},{}) for monitor=({},{})",
        reason, x, y, monitor_position.x, monitor_position.y
    );
}

/// 移动事件在拖拽期间会高频触发，只在停止移动一小段时间后保存最后位置。
fn schedule_window_position_save(window: &tauri::Window, position: (i32, i32)) {
    let Some(state) = window.app_handle().try_state::<SavedWindowPositions>() else {
        return;
    };
    let generation = state.move_generation.fetch_add(1, Ordering::Relaxed) + 1;

    if let Some(pending) = state.pending_move_save.lock().take() {
        pending.abort();
    }

    let window = window.clone();
    let task = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(POSITION_SAVE_DEBOUNCE).await;
        let Some(state) = window.app_handle().try_state::<SavedWindowPositions>() else {
            return;
        };
        if state.move_generation.load(Ordering::Relaxed) != generation {
            return;
        }
        save_window_position(&window, Some(position), "Moved(debounced)");
    });
    state.pending_move_save.lock().replace(task);
}

fn save_position_on_focus_lost(window: &tauri::Window) {
    if let Some(state) = window.app_handle().try_state::<SavedWindowPositions>() {
        state.move_generation.fetch_add(1, Ordering::Relaxed);
        if let Some(pending) = state.pending_move_save.lock().take() {
            pending.abort();
        }
    }
    save_window_position(window, None, "Focused(false)");
}

/// 监听窗口移动与失焦，及时记住用户拖拽后的最后位置。
pub fn setup_window_event_handler(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::Moved(position) => {
            schedule_window_position_save(window, (position.x, position.y));
        }
        tauri::WindowEvent::Focused(false) => save_position_on_focus_lost(window),
        _ => {}
    }
}
