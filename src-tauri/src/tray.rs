// 系统托盘模块

use crate::window::positioning::reposition_to_cursor_monitor;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

/// 显示窗口并稳定抢到前台（跨平台）
///
/// 包装各平台的"绕过前台锁定"实现，确保从托盘打开窗口的体验与快捷键一致。
fn show_window(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    crate::platform::windows::bring_to_front(window);

    #[cfg(not(target_os = "windows"))]
    {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 创建系统托盘
///
/// 在 macOS 上会在屏幕顶部右侧菜单栏常驻一个图标 + "Buddy" 标题，
/// 即使主窗口隐藏时仍然可见。
pub fn create_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let settings_item = MenuItem::with_id(app, "设置", "设置", true, None::<&str>)?;
    let autostart_item = MenuItem::with_id(app, "autostart", "开机自启", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&settings_item, &autostart_item, &quit_item])?;

    // 菜单栏专用图标：纯黑 template style（buddy-menubar.svg 派生）。
    // 编译期 embed 进二进制，运行时无路径依赖。
    // 优先用 @2x (72px) — Retina 屏正确采样；@1x 36px 给低分屏。
    let icon_bytes: &[u8] = include_bytes!("../icons/tray@2x.png");
    let rgba = image::load_from_memory_with_format(
        icon_bytes,
        image::ImageFormat::Png,
    )?
    .to_rgba8();
    let (w, h) = rgba.dimensions();
    let icon = Image::new_owned(rgba.into_raw(), w, h);

    TrayIconBuilder::with_id("buddy-tray")
        .icon(icon)
        .icon_as_template(true) // macOS template 风格：黑透，自动跟随系统深浅色
        .title("Buddy")          // 图标右边显示的文字（macOS 状态栏特有）
        .tooltip("Buddy - AI Chat Assistant")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "设置" => {
                if let Some(window) = app.get_webview_window("main") {
                    reposition_to_cursor_monitor(&window);
                    show_window(&window);
                }
            }
            "autostart" => {}
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
