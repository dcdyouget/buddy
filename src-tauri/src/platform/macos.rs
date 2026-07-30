// macOS 平台特定代码

use tauri::Emitter;

/// 将无 Dock 图标的辅助应用窗口激活到 macOS 前台。
///
/// `show() + set_focus()` 对 `skipTaskbar` 窗口并不稳定：窗口可能已经可见，
/// 但应用本身仍未激活，导致全局快捷键触发后窗口留在其他应用后面。
#[cfg(target_os = "macos")]
pub fn bring_to_front(window: &tauri::WebviewWindow) {
    use log::{info, warn};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSNormalWindowLevel, NSWindow};

    let _ = window.show();
    let task_window = window.clone();
    if let Err(error) = window.run_on_main_thread(move || {
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
        info!(
            "[macos] bring_to_front completed, visible={}, focused={}",
            task_window.is_visible().unwrap_or(false),
            task_window.is_focused().unwrap_or(false)
        );
    }) {
        warn!("[macos] failed to schedule window activation: {error}");
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

    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(_) => return,
    };
    let old_text = clipboard.get_text().unwrap_or_default();

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
    key_down.post(CGEventTapLocation::HID);
    key_up.post(CGEventTapLocation::HID);

    thread::sleep(Duration::from_millis(50));

    if let Ok(new_text) = clipboard.get_text() {
        if !new_text.is_empty() && new_text != old_text {
            let _ = app.emit("selected-text", new_text);
        }
    }

    let _ = clipboard.set_text(old_text);
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
