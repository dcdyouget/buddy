// 应用入口模块：负责窗口创建、快捷键注册、系统托盘、毛玻璃效果及圆角等核心初始化逻辑

use commands::CancelState;
use log::info;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_global_shortcut::Shortcut;

/// 记录窗口最后 show 的时间戳，用于失焦冷却
pub struct LastShowTime(pub Mutex<Instant>);

/// 记录用户拖拽后的窗口位置（物理像素），key 为显示器 (x,y)，value 为窗口位置 (x,y)
pub struct SavedWindowPositions(pub Mutex<std::collections::HashMap<(i32, i32), (i32, i32)>>);

mod commands;
mod hotkey;
mod models;
mod providers;
mod storage;
mod streaming;

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

/// 将窗口重新定位到当前光标所在显示器的中心位置（多屏适配）
///
/// 多屏场景下用户按下快捷键时期望窗口出现在当前聚焦的屏幕上。
/// 此函数获取光标位置 → 找到包含光标的显示器 → 将窗口居中到该显示器。
///
/// macOS 下使用 NSWindow.setFrameOrigin: 同步设置窗口位置，
/// 避免 Tauri set_position() 的异步 IPC 延迟导致 show() 时闪现旧位置。
pub(crate) fn reposition_to_cursor_monitor(window: &tauri::WebviewWindow) {
    // 获取当前光标位置
    let cursor_pos = match window.cursor_position() {
        Ok(pos) => pos,
        Err(_) => return,
    };

    // 遍历所有显示器，找到包含光标的那个
    let monitors = match window.available_monitors() {
        Ok(m) => m,
        Err(_) => return,
    };

    let target_monitor = monitors.iter().find(|m| {
        let m_pos = m.position();
        let m_size = m.size();
        cursor_pos.x >= m_pos.x as f64
            && cursor_pos.x < (m_pos.x + m_size.width as i32) as f64
            && cursor_pos.y >= m_pos.y as f64
            && cursor_pos.y < (m_pos.y + m_size.height as i32) as f64
    });

    // 若找不到包含光标的显示器，回退到当前显示器或主显示器
    let monitor = match target_monitor {
        Some(m) => m.clone(),
        None => match window.current_monitor() {
            Ok(Some(m)) => m,
            _ => match window.primary_monitor() {
                Ok(Some(m)) => m,
                _ => return,
            },
        },
    };

    let m_pos = monitor.position();
    let m_size = monitor.size();
    let w_size = match window.outer_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let scale = monitor.scale_factor();

    // 统一转换为逻辑坐标（点）
    let m_x = m_pos.x as f64 / scale;
    let m_y = m_pos.y as f64 / scale;
    let m_w = m_size.width as f64 / scale;
    let m_h = m_size.height as f64 / scale;
    let w_w = w_size.width as f64 / scale;
    let w_h = w_size.height as f64 / scale;

    // 默认：屏幕居中。如果用户拖动过窗口，则使用记住的位置
    let mut x = m_x + (m_w - w_w) / 2.0;
    let mut y = m_y + (m_h - w_h) / 2.0;

    // 检查是否有用户记住的位置（同一显示器）
    {
        let saved = window
            .app_handle()
            .try_state::<SavedWindowPositions>()
            .and_then(|s| s.0.lock().ok().and_then(|map| map.get(&(m_pos.x, m_pos.y)).copied()));
        if let Some((sx, sy)) = saved {
            // 确保记住的位置仍在当前屏幕范围内
            let sxf = sx as f64 / scale;
            let syf = sy as f64 / scale;
            if sxf >= m_x && sxf + w_w <= m_x + m_w && syf >= m_y && syf + w_h <= m_y + m_h {
                x = sxf;
                y = syf;
                log::info!("[reposition] using saved position: ({:.0},{:.0}) logical", x, y);
            }
        }
    }

    // macOS 坐标系转换：(0,0) 在主屏左下角，需要把我们的左上角 Y 转成左下角 Y
    // 公式：macos_y = 主屏逻辑高度 - 左上角_y - 窗口逻辑高度
    #[cfg(target_os = "macos")]
    let macos_y = {
        let primary_logical_h = window
            .primary_monitor()
            .ok()
            .flatten()
            .map(|m| m.size().height as f64 / m.scale_factor())
            .unwrap_or(m_y + m_h); // 回退：用目标屏幕的底边
        primary_logical_h - y - w_h
    };
    #[cfg(not(target_os = "macos"))]
    let macos_y = 0.0_f64;

    info!("[reposition] final logical pos=({:.0},{:.0}) macosY={:.0}, win_logical=({:.0}x{:.0})", x, y, macos_y, w_w, w_h);

    // macOS：使用 NSWindow.setFrameOrigin: 同步设置位置（macOS 左下角坐标），防止跨屏闪烁
    #[cfg(target_os = "macos")]
    {
        if let Ok(ns_window_ptr) = window.ns_window() {
            use objc2::msg_send;
            use objc2::runtime::AnyObject;
            use objc2_foundation::NSPoint;
            unsafe {
                let ns_window: *mut AnyObject = ns_window_ptr as *mut _;
                let point = NSPoint::new(x, macos_y);
                let _: () = msg_send![ns_window, setFrameOrigin: point];
            }
            return;
        }
    }

    // 非 macOS 回退：Tauri API 需要物理像素
    let _ = window.set_position(tauri::PhysicalPosition::new(
        (x * scale) as i32,
        (y * scale) as i32,
    ));
}

/// 标记窗口刚被 show，更新失焦冷却时间戳
///
/// 在 hotkey/tray 等所有 show 窗口的入口处调用，
/// 防止 Focused(false) 事件在 show 后 200ms 内误隐藏窗口（跨屏呼出时常见）。
pub(crate) fn mark_window_shown(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LastShowTime>() {
        if let Ok(mut t) = state.0.lock() {
            *t = Instant::now();
            log::info!("[mark_window_shown] timestamp updated");
        }
    }
}

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
        // 记录窗口 show 时间戳，用于失焦冷却（防跨屏呼出时被误隐藏）
        .manage(LastShowTime(Mutex::new(Instant::now())))
        // 记录用户拖拽后的窗口位置（按显示器记忆）
        .manage(SavedWindowPositions(Mutex::new(std::collections::HashMap::new())))
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
                            reposition_to_cursor_monitor(&window);
                            let _ = window.show();
                            mark_window_shown(app);
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
                            reposition_to_cursor_monitor(&window);
                            let _ = window.show();
                            mark_window_shown(app);
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
        // 加 200ms 冷却：防止跨屏呼出时 show/setFocus 之间 macOS 短暂失焦导致窗口被误隐藏
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Focused(false) => {
                    let now = std::time::Instant::now();
                    let last_show = window
                        .app_handle()
                        .try_state::<LastShowTime>()
                        .and_then(|s| s.0.lock().ok().map(|t| *t));
                    let in_cooldown = last_show
                        .map(|t| now.duration_since(t) < std::time::Duration::from_millis(200))
                        .unwrap_or(false);
                    log::info!(
                        "[on_window_event] Focused(false), in_cooldown={}, last_show={:?}",
                        in_cooldown,
                        last_show.map(|t| format!("{:?} ago", now.duration_since(t)))
                    );
                    if in_cooldown {
                        return;
                    }
                    // 隐藏前保存窗口位置（用户可能拖拽过），下次同屏呼出时恢复
                    if let Ok(pos) = window.outer_position() {
                        if let Ok(Some(mon)) = window.current_monitor() {
                            let mp = mon.position();
                            if let Some(state) = window.app_handle().try_state::<SavedWindowPositions>() {
                                if let Ok(mut map) = state.0.lock() {
                                    map.insert((mp.x, mp.y), (pos.x, pos.y));
                                    log::info!("[on_window_event] saved pos=({},{}) for monitor=({},{})", pos.x, pos.y, mp.x, mp.y);
                                }
                            }
                        }
                    }
                    log::info!("[on_window_event] hiding window");
                    let _ = window.hide();
                    log::info!("[on_window_event] hidden, visible={}", window.is_visible().unwrap_or(false));
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
