// 系统托盘模块

use crate::window::events::mark_window_shown;
use crate::window::positioning::reposition_to_cursor_monitor;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

/// 创建系统托盘菜单
pub fn create_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let settings_item = MenuItem::with_id(app, "设置", "设置", true, None::<&str>)?;
    let autostart_item = MenuItem::with_id(app, "autostart", "开机自启", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&settings_item, &autostart_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "设置" => {
                if let Some(window) = app.get_webview_window("main") {
                    reposition_to_cursor_monitor(&window);
                    let _ = window.show();
                    mark_window_shown(app);
                    let _ = window.set_focus();
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
                    let _ = window.show();
                    mark_window_shown(app);
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
