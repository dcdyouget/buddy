// 系统托盘模块

use crate::{storage, window::positioning::reposition_to_cursor_monitor};
use log::{error, warn};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_autostart::ManagerExt;

/// 显示窗口并稳定抢到前台（跨平台）
///
/// 包装各平台的"绕过前台锁定"实现，确保从托盘打开窗口的体验与快捷键一致。
fn show_window(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    crate::platform::windows::bring_to_front(window);

    #[cfg(target_os = "macos")]
    crate::platform::macos::bring_to_front(window);

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 将系统自启状态同步到 config.json，并通知已打开的前端刷新配置。
fn sync_auto_start_config(app: &tauri::AppHandle, enabled: bool) {
    match storage::get_config(app) {
        Ok(mut config) => {
            config.auto_start = enabled;
            if let Err(err) = storage::save_config(app, &config) {
                error!("[tray] 保存开机自启配置失败: {}", err);
            }
        }
        Err(err) => warn!("[tray] 读取配置失败，无法同步开机自启状态: {}", err),
    }

    if let Err(err) = app.emit("auto-start-changed", enabled) {
        warn!("[tray] 通知前端刷新开机自启状态失败: {}", err);
    }
}

/// 创建系统托盘
///
/// 在 macOS 上会在屏幕顶部右侧菜单栏常驻一个图标 + "Buddy" 标题，
/// 即使主窗口隐藏时仍然可见。
pub fn create_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let stored_auto_start = storage::get_config(app.handle())
        .map(|config| config.auto_start)
        .unwrap_or(false);
    let auto_start_enabled = app.autolaunch().is_enabled().unwrap_or(stored_auto_start);
    sync_auto_start_config(app.handle(), auto_start_enabled);

    let autostart_item = CheckMenuItem::with_id(
        app,
        "autostart",
        "开机自启",
        true,
        auto_start_enabled,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&settings_item, &autostart_item, &separator, &quit_item],
    )?;
    let autostart_item_for_menu = autostart_item.clone();

    // 菜单栏专用图标：纯黑 template style（buddy-menubar.svg 派生）。
    // 编译期 embed 进二进制，运行时无路径依赖。
    // 优先用 @2x (72px) — Retina 屏正确采样；@1x 36px 给低分屏。
    let icon_bytes: &[u8] = include_bytes!("../icons/tray@2x.png");
    let rgba = image::load_from_memory_with_format(icon_bytes, image::ImageFormat::Png)?.to_rgba8();
    let (w, h) = rgba.dimensions();
    let icon = Image::new_owned(rgba.into_raw(), w, h);

    TrayIconBuilder::with_id("buddy-tray")
        .icon(icon)
        .icon_as_template(true) // macOS template 风格：黑透，自动跟随系统深浅色
        .title("Buddy") // 图标右边显示的文字（macOS 状态栏特有）
        .tooltip("Buddy - AI Chat Assistant")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    reposition_to_cursor_monitor(&window);
                    show_window(&window);
                    if let Err(err) = app.emit("open-settings", ()) {
                        warn!("[tray] 打开设置页事件发送失败: {}", err);
                    }
                }
            }
            "autostart" => {
                let stored = storage::get_config(app)
                    .map(|config| config.auto_start)
                    .unwrap_or(false);
                let current = app.autolaunch().is_enabled().unwrap_or(stored);
                let next = !current;
                let result = if next {
                    app.autolaunch().enable()
                } else {
                    app.autolaunch().disable()
                };

                match result {
                    Ok(()) => {
                        if let Err(err) = autostart_item_for_menu.set_checked(next) {
                            warn!("[tray] 更新开机自启菜单勾选状态失败: {}", err);
                        }
                        sync_auto_start_config(app, next);
                    }
                    Err(err) => {
                        error!("[tray] 切换开机自启失败: {}", err);
                        let _ = autostart_item_for_menu.set_checked(current);
                    }
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    reposition_to_cursor_monitor(&window);
                    show_window(&window);
                }
            }
        })
        .build(app)?;

    Ok(())
}
