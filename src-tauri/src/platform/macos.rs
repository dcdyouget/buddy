// macOS 平台特定代码

use tauri::Emitter;

/// 将无 Dock 图标的辅助应用窗口激活到 macOS 前台。
///
/// `show() + set_focus()` 对 `skipTaskbar` 窗口并不稳定：窗口可能已经可见，
/// 但应用本身仍未激活，导致全局快捷键触发后窗口留在其他应用后面。
#[cfg(target_os = "macos")]
pub fn bring_to_front(window: &tauri::WebviewWindow) {
    bring_to_front_impl(window, None);
}

/// 带快捷键调用链 ID 的前台激活，便于在发布日志中关联定位与实际显示延迟。
#[cfg(target_os = "macos")]
pub fn bring_to_front_traced(window: &tauri::WebviewWindow, trace_id: u64) {
    bring_to_front_impl(window, Some(trace_id));
}

#[cfg(target_os = "macos")]
fn bring_to_front_impl(window: &tauri::WebviewWindow, trace_id: Option<u64>) {
    use log::{info, warn};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSNormalWindowLevel, NSWindow};
    use std::time::Instant;

    let started_at = Instant::now();
    let show_started_at = Instant::now();
    let show_result = window.show();
    let show_elapsed = show_started_at.elapsed();
    let task_window = window.clone();
    let queued_at = Instant::now();
    if let Err(error) = window.run_on_main_thread(move || {
        let main_queue_elapsed = queued_at.elapsed();
        let activation_started_at = Instant::now();
        if let Ok(ns_window_ptr) = task_window.ns_window() {
            unsafe {
                let ns_window = &*(ns_window_ptr as *const NSWindow);
                let marker = MainThreadMarker::new()
                    .expect("macOS window activation must run on the main thread");
                let application = NSApplication::sharedApplication(marker);
                #[allow(deprecated)]
                application.activateIgnoringOtherApps(true);
                // 只在本次呼出时置前；普通层级保证切换到其他应用后
                // Buddy 会自然退到后面，而不是持续置顶。
                ns_window.setLevel(NSNormalWindowLevel);
                ns_window.orderFrontRegardless();
                ns_window.makeKeyAndOrderFront(None);
            }
        }
        let _ = task_window.set_focus();
        let activation_elapsed = activation_started_at.elapsed();
        let total_elapsed = started_at.elapsed();
        let visible = task_window.is_visible().unwrap_or(false);
        let focused = task_window.is_focused().unwrap_or(false);
        info!(
            "[macos] bring_to_front completed, visible={}, focused={}",
            visible, focused
        );
        let timing = format!(
            "[window-diag] id={} activate-main total={}ms, show={}ms, show_ok={}, main_queue={}ms, native={}ms, visible={visible}, focused={focused}, pos={:?}",
            trace_id.map_or_else(|| "tray".to_string(), |id| id.to_string()),
            total_elapsed.as_millis(),
            show_elapsed.as_millis(),
            show_result.is_ok(),
            main_queue_elapsed.as_millis(),
            activation_elapsed.as_millis(),
            task_window.outer_position().ok(),
        );
        crate::window::log_diagnostic(&timing);

        // set_focus/makeKeyAndOrderFront 可能在当前主线程回调结束后才真正生效。
        // 延迟采样能区分“调用已返回”和“窗口实际可见/获得焦点”之间的系统排队。
        let verification_window = task_window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            crate::window::log_diagnostic(&format!(
                "[window-diag] id={} activate-check elapsed={}ms, visible={}, focused={}, pos={:?}, size={:?}",
                trace_id.map_or_else(|| "tray".to_string(), |id| id.to_string()),
                started_at.elapsed().as_millis(),
                verification_window.is_visible().unwrap_or(false),
                verification_window.is_focused().unwrap_or(false),
                verification_window.outer_position().ok(),
                verification_window.outer_size().ok(),
            ));
        });
    }) {
        warn!("[macos] failed to schedule window activation: {error}");
        crate::window::log_diagnostic(&format!(
            "[window-diag] id={} activate-schedule-failed elapsed={}ms, error={error}",
            trace_id.map_or_else(|| "tray".to_string(), |id| id.to_string()),
            started_at.elapsed().as_millis(),
        ));
    }
}

/// macOS 平台：模拟 Cmd+C 复制当前选中文本（Bob 风格取词）
#[cfg(target_os = "macos")]
pub fn get_selected_text(app: &tauri::AppHandle) {
    use arboard::Clipboard;
    use core_graphics::event::{CGEvent, CGEventTapLocation};
    use core_graphics::event_source::CGEventSource;
    use std::thread;
    use std::time::Duration;

    // 读旧剪贴板必须发生在模拟 Cmd+C 之前，否则读到的是复制后的新内容
    let old_text = Clipboard::new()
        .ok()
        .and_then(|mut clipboard| clipboard.get_text().ok())
        .unwrap_or_default();

    let source = CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::Private)
        .unwrap_or_else(|_| {
            CGEventSource::new(
                core_graphics::event_source::CGEventSourceStateID::CombinedSessionState,
            )
            .unwrap()
        });
    let flags = core_graphics::event::CGEventFlags::CGEventFlagCommand;

    let key_down = CGEvent::new_keyboard_event(source.clone(), 8, true).unwrap();
    let key_up = CGEvent::new_keyboard_event(source, 8, false).unwrap();
    key_down.set_flags(flags);
    key_up.set_flags(flags);
    // 模拟 Cmd+C 必须同步发出：此刻目标应用仍处于前台
    key_down.post(CGEventTapLocation::HID);
    key_up.post(CGEventTapLocation::HID);

    // 「等待剪贴板更新 + 读取 + 恢复」放到后台线程：
    // 热键回调运行在主线程，同步 sleep 50ms 会让每次唤出窗口时 UI 卡一下。
    // 剪贴板（general pasteboard）是线程安全的，可在后台读取。
    let app_handle = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        if let Ok(new_text) = Clipboard::new().and_then(|mut c| c.get_text()) {
            if !new_text.is_empty() && new_text != old_text {
                // 前端收到后也会 trim；这里直接发 trim 后的文本保持一致
                let _ = app_handle.emit("selected-text", new_text.trim().to_string());
            }
        }
        // 恢复旧剪贴板（即使读取失败也恢复，避免弄丢用户剪贴板内容）
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(old_text);
        }
    });
}

/// 非 macOS 平台的取词空实现
#[cfg(not(target_os = "macos"))]
pub fn get_selected_text(_app: &tauri::AppHandle) {}

/// macOS 平台：应用无边框窗口圆角。
///
/// Buddy 当前使用实色白色界面；不启用原生 vibrancy，避免页面切换中
/// 透明区域短暂露出 HUDWindow 的灰色底。
#[cfg(target_os = "macos")]
pub fn apply_macos_window_effects(window: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    if let Ok(ns_window_ptr) = window.ns_window() {
        unsafe {
            let ns_window: *mut AnyObject = ns_window_ptr as *mut _;
            let content_view: *mut AnyObject = msg_send![ns_window, contentView];
            let _: () = msg_send![content_view, setWantsLayer: true];
            let layer: *mut AnyObject = msg_send![content_view, layer];
            let _: () = msg_send![layer, setCornerRadius: 16.0f64];
            let _: () = msg_send![layer, setMasksToBounds: true];
            let _: () = msg_send![ns_window, setHasShadow: false];
        }
    }
}

/// 非 macOS 平台的 vibrancy 空实现
#[cfg(not(target_os = "macos"))]
pub fn apply_macos_window_effects(_window: &tauri::WebviewWindow) {}
