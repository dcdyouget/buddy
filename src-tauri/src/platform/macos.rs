// macOS 平台特定代码

use tauri::Emitter;

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
        .unwrap_or_else(|_| CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::CombinedSessionState).unwrap());
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
