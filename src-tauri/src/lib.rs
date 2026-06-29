// 应用入口模块：负责窗口创建、快捷键注册、系统托盘、毛玻璃效果及圆角等核心初始化逻辑

use commands::CancelState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_global_shortcut::Shortcut;

mod api;
mod commands;
mod hotkey;
mod models;
mod storage;

/// macOS 平台：模拟 Cmd+C 复制当前选中文本（Bob 风格取词）
///
/// 流程：
/// 1. 保存当前剪贴板内容
/// 2. 通过 CoreGraphics 发送 Cmd+C 按键事件
/// 3. 等待复制完成
/// 4. 读取新剪贴板内容，若非空且不同于旧内容则通过事件发送给前端
/// 5. 恢复原始剪贴板内容
#[cfg(target_os = "macos")]
pub(crate) fn get_selected_text(app: &tauri::AppHandle) {
    use arboard::Clipboard;
    use core_graphics::event::{CGEvent, CGEventTapLocation};
    use core_graphics::event_source::CGEventSource;
    use std::thread;
    use std::time::Duration;

    // 1. 保存当前剪贴板内容
    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(_) => return,
    };
    let old_text = clipboard.get_text().unwrap_or_default();

    // 2. 模拟 Cmd+C 按键
    let source = CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::Private)
        .unwrap_or_else(|_| CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::CombinedSessionState).unwrap());
    let flags = core_graphics::event::CGEventFlags::CGEventFlagCommand;

    // keycode 8 = C 键
    let key_down = CGEvent::new_keyboard_event(source.clone(), 8, true).unwrap();
    let key_up = CGEvent::new_keyboard_event(source, 8, false).unwrap();
    key_down.set_flags(flags);
    key_up.set_flags(flags);
    key_down.post(CGEventTapLocation::HID); // 发送到 HID 层级
    key_up.post(CGEventTapLocation::HID);

    // 3. 等待复制完成
    thread::sleep(Duration::from_millis(50));

    // 4. 读取新剪贴板内容
    if let Ok(new_text) = clipboard.get_text() {
        if !new_text.is_empty() && new_text != old_text {
            // 发送 selected-text 事件给前端
            let _ = app.emit("selected-text", new_text);
        }
    }

    // 5. 恢复原始剪贴板
    let _ = clipboard.set_text(old_text);
}

/// 非 macOS 平台的取词函数空实现
#[cfg(not(target_os = "macos"))]
pub(crate) fn get_selected_text(_app: &tauri::AppHandle) {}

/// 应用主入口 run 函数：配置并启动整个 Tauri 应用
///
/// 注册插件、设置快捷键、创建系统托盘、应用窗口视觉效果、绑定 IPC 命令处理器，
/// 并监听窗口失焦事件以实现「点击外部关闭窗口」的行为。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 自动启动插件（macOS LaunchAgent 方式）
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        // 全局快捷键插件
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // 注册取消流式生成的共享状态
        .manage(CancelState {
            sender: std::sync::Mutex::new(None),
        })
        // 注册快捷键追踪状态（用于更新时注销旧热键）
        .manage(hotkey::HotkeyState {
            current: std::sync::Mutex::new(None),
        })
        .setup(|app| {
            // 从磁盘加载用户配置，沿用默认值作为兜底
            let config = storage::get_config(app.handle())
                .unwrap_or_else(|_| models::AppConfig::default());

            // 注册全局快捷键：解析用户配置的热键字符串
            let shortcut: Shortcut = config
                .hotkey
                .parse()
                .unwrap_or(Shortcut::new(None, tauri_plugin_global_shortcut::Code::KeyJ));

            // 记录当前快捷键到共享状态（供后续更新时注销旧热键使用）
            app.state::<hotkey::HotkeyState>()
                .current
                .lock()
                .unwrap()
                .replace(shortcut);

            // 注册快捷键（窗口 toggle + 取词行为）
            hotkey::register(app.handle(), shortcut);

            // 创建系统托盘菜单
            let settings_item =
                MenuItem::with_id(app, "设置", "设置", true, None::<&str>)?;
            let autostart_item =
                MenuItem::with_id(app, "autostart", "开机自启", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&settings_item, &autostart_item, &quit_item])?;

            // 构建并注册系统托盘图标
            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "设置" => {
                        // 点击「设置」→ 显示主窗口
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "autostart" => {} // 开机自启选项（预留）
                    "quit" => {
                        // 点击「退出」→ 退出应用
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // 左键点击托盘图标 → 显示主窗口
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // macOS 平台：应用毛玻璃效果和圆角
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                // 应用 HUD 风格的毛玻璃效果（深色半透明背景）
                let _ = window_vibrancy::apply_vibrancy(
                    &window,
                    window_vibrancy::NSVisualEffectMaterial::HudWindow,
                    Some(window_vibrancy::NSVisualEffectState::Active),
                    None,
                );

                // 通过 objc2 直接设置原生窗口圆角（16px）
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
                        let _: () = msg_send![ns_window, setHasShadow: false]; // 用前端 CSS 阴影替代原生阴影
                    }
                }
            }

            Ok(())
        })
        // 注册所有 IPC 命令处理器（前端 invoke → 后端处理）
        .invoke_handler(tauri::generate_handler![
            commands::send_message,
            commands::stop_generation,
            commands::get_config,
            commands::save_config,
            commands::fetch_models,
            commands::test_latency,
            commands::load_messages,
            commands::save_message,
        ])
        // 窗口失焦时自动隐藏（实现「点击外部关闭」行为）
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(false) = event {
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
